use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use sqlx::{Row, SqliteExecutor, SqlitePool};
use tokio::sync::watch;

use ac_platform::Secrets;

use crate::db::{self, seal, unseal};
use crate::error::Result;
use crate::jwt::Claims;
use crate::migrate_legacy::LegacyPaths;
use crate::model::{Account, Group, LaunchOpts, Settings};

const ONLINE_FLUSH_INTERVAL: i64 = 60;
const MAX_GROUP_NAME: usize = 24;

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

#[derive(Default)]
struct VaultState {
    accounts: HashMap<String, Account>,
    forgotten: HashMap<String, String>,
    groups: Vec<Group>,
}

pub struct Store {
    pub(crate) pool: SqlitePool,
    pub(crate) secrets: Arc<Secrets>,
    vault: RwLock<VaultState>,
    settings: RwLock<Settings>,
    revision: watch::Sender<u64>,
    online_flush: AtomicI64,
}

impl Store {
    pub async fn open(db_path: &Path, secrets: Arc<Secrets>, legacy: LegacyPaths) -> Result<Self> {
        let fresh = !db_path.exists();
        let pool = db::connect(db_path).await?;
        if fresh && let Err(err) = crate::migrate_legacy::import(&pool, &secrets, &legacy).await {
            pool.close().await;
            let _ = std::fs::remove_file(db_path);
            return Err(err);
        }
        crate::uuid::repair(&pool).await?;
        let store = Self {
            pool,
            secrets,
            vault: RwLock::new(VaultState::default()),
            settings: RwLock::new(Settings::default()),
            revision: watch::channel(0).0,
            online_flush: AtomicI64::new(0),
        };
        store.reload().await?;
        Ok(store)
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }

    fn bump(&self) {
        self.revision.send_modify(|revision| *revision += 1);
    }

    async fn reload(&self) -> Result<()> {
        let mut state = VaultState::default();
        for row in sqlx::query("SELECT * FROM accounts")
            .fetch_all(&self.pool)
            .await?
        {
            let account = self.row_to_account(&row)?;
            state.accounts.insert(account.uuid.clone(), account);
        }
        for row in sqlx::query("SELECT uuid, token FROM forgotten")
            .fetch_all(&self.pool)
            .await?
        {
            let uuid: String = row.try_get("uuid")?;
            let token = unseal(&self.secrets, row.try_get("token")?)?;
            state.forgotten.insert(uuid, token);
        }
        state.groups = self.load_groups().await?;
        *self.settings.write() = self.load_settings().await?;
        *self.vault.write() = state;
        Ok(())
    }

    fn row_to_account(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Account> {
        Ok(Account {
            uuid: row.try_get("uuid")?,
            name: row.try_get("name")?,
            token: unseal(&self.secrets, row.try_get("token")?)?,
            label: row.try_get("label")?,
            pinned: row.try_get::<i64, _>("pinned")? != 0,
            client: row.try_get("client")?,
            profile: row.try_get("profile")?,
            ram: row.try_get("ram")?,
            min_graphics: row.try_get::<i64, _>("min_graphics")? != 0,
            fullscreen: row.try_get::<i64, _>("fullscreen")? != 0,
            discord_rpc: row.try_get::<i64, _>("discord_rpc")? != 0,
            auto_enter: row.try_get::<i64, _>("auto_enter")? != 0,
            debug_mode: row.try_get::<i64, _>("debug_mode")? != 0,
            render_distance: row.try_get("render_distance")?,
            max_fps: row.try_get("max_fps")?,
            animations: row.try_get("animations")?,
            fast_render: row.try_get("fast_render")?,
            minimal: row.try_get::<i64, _>("minimal")? != 0,
            expires: row.try_get("expires")?,
            first_seen: row.try_get("first_seen")?,
            last_seen: row.try_get("last_seen")?,
            last_launched: row.try_get("last_launched")?,
            last_online: row.try_get("last_online")?,
        })
    }

    pub(crate) async fn persist_account(&self, account: &Account) -> Result<()> {
        self.write_account(&self.pool, account).await
    }

    async fn write_account<'e>(
        &self,
        executor: impl SqliteExecutor<'e>,
        account: &Account,
    ) -> Result<()> {
        let token = seal(&self.secrets, &account.token)?;
        sqlx::query(
            "INSERT INTO accounts (uuid, name, token, label, pinned, client, profile, ram, \
             min_graphics, fullscreen, discord_rpc, auto_enter, debug_mode, render_distance, \
             max_fps, animations, fast_render, minimal, expires, first_seen, last_seen, \
             last_launched, last_online) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
             ?18, ?19, ?20, ?21, ?22, ?23) \
             ON CONFLICT (uuid) DO UPDATE SET name = ?2, token = ?3, label = ?4, pinned = ?5, \
             client = ?6, profile = ?7, ram = ?8, min_graphics = ?9, fullscreen = ?10, \
             discord_rpc = ?11, auto_enter = ?12, debug_mode = ?13, render_distance = ?14, \
             max_fps = ?15, animations = ?16, fast_render = ?17, minimal = ?18, expires = ?19, \
             first_seen = ?20, last_seen = ?21, last_launched = ?22, last_online = ?23",
        )
        .bind(&account.uuid)
        .bind(&account.name)
        .bind(token)
        .bind(&account.label)
        .bind(i64::from(account.pinned))
        .bind(&account.client)
        .bind(&account.profile)
        .bind(account.ram)
        .bind(i64::from(account.min_graphics))
        .bind(i64::from(account.fullscreen))
        .bind(i64::from(account.discord_rpc))
        .bind(i64::from(account.auto_enter))
        .bind(i64::from(account.debug_mode))
        .bind(account.render_distance)
        .bind(account.max_fps)
        .bind(account.animations)
        .bind(account.fast_render)
        .bind(i64::from(account.minimal))
        .bind(account.expires)
        .bind(account.first_seen)
        .bind(account.last_seen)
        .bind(account.last_launched)
        .bind(account.last_online)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub fn get(&self, uuid: &str) -> Option<Account> {
        self.vault.read().accounts.get(uuid).cloned()
    }

    pub fn list(&self) -> Vec<Account> {
        let mut out: Vec<Account> = self.vault.read().accounts.values().cloned().collect();
        out.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.last_launched.cmp(&a.last_launched))
                .then(b.last_seen.cmp(&a.last_seen))
                .then(a.name.cmp(&b.name))
        });
        out
    }

    pub async fn upsert_token(&self, name: &str, token: &str, claims: &Claims) -> Result<bool> {
        let account = {
            let mut state = self.vault.write();
            if let Some(previous) = state.forgotten.get(&claims.uuid) {
                if previous == token {
                    return Ok(false);
                }
                state.forgotten.remove(&claims.uuid);
            }
            let now = now();
            let account = state
                .accounts
                .entry(claims.uuid.clone())
                .or_insert_with(|| Account {
                    uuid: claims.uuid.clone(),
                    first_seen: now,
                    ..Account::default()
                });
            let mut changed = false;
            if !name.is_empty() && account.name != name {
                account.name = name.to_owned();
                changed = true;
            }
            if !token.is_empty() && account.token != token && claims.exp >= account.expires {
                account.token = token.to_owned();
                account.expires = claims.exp;
                changed = true;
            }
            account.last_seen = now;
            if !changed {
                return Ok(false);
            }
            account.clone()
        };
        self.remove_forgotten(&claims.uuid).await?;
        self.persist_account(&account).await?;
        self.bump();
        Ok(true)
    }

    async fn remove_forgotten(&self, uuid: &str) -> Result<()> {
        sqlx::query("DELETE FROM forgotten WHERE uuid = ?1")
            .bind(uuid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_account<F>(&self, uuid: &str, mutate: F) -> Result<bool>
    where
        F: FnOnce(&mut Account) -> bool,
    {
        let account = {
            let mut state = self.vault.write();
            let Some(account) = state.accounts.get_mut(uuid) else {
                return Ok(false);
            };
            if !mutate(account) {
                return Ok(false);
            }
            account.clone()
        };
        self.persist_account(&account).await?;
        self.bump();
        Ok(true)
    }

    async fn update_many<F>(&self, uuids: &[String], mutate: F) -> Result<bool>
    where
        F: Fn(&mut Account),
    {
        let changed = {
            let mut state = self.vault.write();
            let mut touched = Vec::new();
            for uuid in uuids {
                if let Some(account) = state.accounts.get_mut(uuid) {
                    mutate(account);
                    touched.push(account.clone());
                }
            }
            touched
        };
        if changed.is_empty() {
            return Ok(false);
        }
        let mut tx = self.pool.begin().await?;
        for account in &changed {
            self.write_account(&mut *tx, account).await?;
        }
        tx.commit().await?;
        self.bump();
        Ok(true)
    }

    pub async fn set_label(&self, uuid: &str, label: &str) -> Result<bool> {
        self.update_account(uuid, |account| {
            account.label = label.to_owned();
            true
        })
        .await
    }

    pub async fn set_pinned(&self, uuid: &str, pinned: bool) -> Result<bool> {
        self.update_account(uuid, |account| {
            account.pinned = pinned;
            true
        })
        .await
    }

    pub async fn set_client(&self, uuid: &str, client: &str) -> Result<bool> {
        if client.is_empty() {
            return Ok(false);
        }
        self.update_account(uuid, |account| {
            account.client = client.to_owned();
            true
        })
        .await
    }

    pub async fn set_profile(&self, uuid: &str, profile: &str) -> Result<bool> {
        self.update_account(uuid, |account| {
            account.profile = profile.to_owned();
            true
        })
        .await
    }

    pub async fn set_ram(&self, uuids: &[String], ram: i64) -> Result<bool> {
        self.update_many(uuids, |account| account.ram = ram).await
    }

    pub async fn set_launch_settings(&self, uuids: &[String], opts: LaunchOpts) -> Result<bool> {
        self.update_many(uuids, |account| {
            account.ram = opts.ram;
            account.min_graphics = opts.min_graphics;
            account.fullscreen = opts.fullscreen;
            account.discord_rpc = opts.discord_rpc;
            account.auto_enter = opts.auto_enter;
            account.debug_mode = opts.debug_mode;
            account.render_distance = opts.render_distance;
            account.max_fps = opts.max_fps;
            account.animations = opts.animations;
            account.fast_render = opts.fast_render;
            account.minimal = opts.minimal;
        })
        .await
    }

    pub async fn mark_launched(&self, uuid: &str) -> Result<bool> {
        let stamp = now();
        self.update_account(uuid, |account| {
            account.last_launched = stamp;
            true
        })
        .await
    }

    pub async fn touch_online(&self, uuids: &[String]) -> Result<()> {
        if uuids.is_empty() {
            return Ok(());
        }
        let now = now();
        let flush = {
            let mut state = self.vault.write();
            let mut touched = false;
            for uuid in uuids {
                if let Some(account) = state.accounts.get_mut(uuid) {
                    account.last_online = now;
                    touched = true;
                }
            }
            touched && now - self.online_flush.load(Ordering::Relaxed) >= ONLINE_FLUSH_INTERVAL
        };
        if !flush {
            return Ok(());
        }
        self.online_flush.store(now, Ordering::Relaxed);
        let snapshot: Vec<(String, i64, i64)> = {
            let state = self.vault.read();
            uuids
                .iter()
                .filter_map(|uuid| state.accounts.get(uuid))
                .map(|account| (account.uuid.clone(), account.last_seen, account.last_online))
                .collect()
        };
        let mut tx = self.pool.begin().await?;
        for (uuid, last_seen, last_online) in &snapshot {
            sqlx::query("UPDATE accounts SET last_seen = ?2, last_online = ?3 WHERE uuid = ?1")
                .bind(uuid)
                .bind(last_seen)
                .bind(last_online)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        self.bump();
        Ok(())
    }

    pub async fn forget(&self, uuid: &str) -> Result<()> {
        let token = {
            let mut state = self.vault.write();
            let token = state
                .accounts
                .remove(uuid)
                .map(|account| account.token)
                .unwrap_or_default();
            state.forgotten.insert(uuid.to_owned(), token.clone());
            for group in &mut state.groups {
                group.members.retain(|member| member != uuid);
            }
            token
        };
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM accounts WHERE uuid = ?1")
            .bind(uuid)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT OR REPLACE INTO forgotten (uuid, token) VALUES (?1, ?2)")
            .bind(uuid)
            .bind(seal(&self.secrets, &token)?)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.bump();
        Ok(())
    }

    pub async fn set_cabinet_session(&self, uuid: &str, session: &str) -> Result<()> {
        let saved = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs() as i64);
        sqlx::query(
            "INSERT INTO account_auth (uuid, cabinet_session, session_saved) VALUES (?1, ?2, ?3) \
             ON CONFLICT (uuid) DO UPDATE SET cabinet_session = ?2, session_saved = ?3",
        )
        .bind(uuid)
        .bind(seal(&self.secrets, session)?)
        .bind(saved)
        .execute(&self.pool)
        .await?;
        self.bump();
        Ok(())
    }

    pub async fn cabinet_session(&self, uuid: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT cabinet_session FROM account_auth WHERE uuid = ?1")
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => {
                let session = unseal(&self.secrets, row.try_get("cabinet_session")?)?;
                Ok((!session.is_empty()).then_some(session))
            }
            None => Ok(None),
        }
    }

    pub async fn cabinet_sessions(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT uuid FROM account_auth WHERE cabinet_session IS NOT NULL")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| Ok(row.try_get("uuid")?))
            .collect()
    }

    pub async fn set_password(&self, uuid: &str, password: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO account_auth (uuid, password) VALUES (?1, ?2) \
             ON CONFLICT (uuid) DO UPDATE SET password = ?2",
        )
        .bind(uuid)
        .bind(seal(&self.secrets, password)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn password(&self, uuid: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT password FROM account_auth WHERE uuid = ?1")
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => {
                let password = unseal(&self.secrets, row.try_get("password")?)?;
                Ok((!password.is_empty()).then_some(password))
            }
            None => Ok(None),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().clone()
    }

    pub(crate) fn settings_lock(&self) -> &RwLock<Settings> {
        &self.settings
    }

    pub(crate) fn bump_revision(&self) {
        self.bump();
    }

    pub(crate) fn with_groups<T>(&self, run: impl FnOnce(&mut Vec<Group>) -> T) -> T {
        run(&mut self.vault.write().groups)
    }

    pub(crate) fn read_groups<T>(&self, run: impl FnOnce(&[Group]) -> T) -> T {
        run(&self.vault.read().groups)
    }

    pub(crate) fn account_exists(&self, uuid: &str) -> bool {
        self.vault.read().accounts.contains_key(uuid)
    }

    pub(crate) const fn max_group_name() -> usize {
        MAX_GROUP_NAME
    }
}
