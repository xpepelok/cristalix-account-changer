use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use sqlx::SqlitePool;

use ac_platform::Secrets;

use crate::db::{pack_pids, seal};
use crate::error::Result;
use crate::model::{Account, Group, LauncherKind};

#[derive(Clone, Debug, Default)]
pub struct LegacyPaths {
    pub vault: PathBuf,
    pub config: PathBuf,
    pub players: PathBuf,
    pub session: PathBuf,
    pub logs: PathBuf,
}

#[derive(Deserialize)]
struct VaultData {
    #[serde(default)]
    accounts: HashMap<String, Account>,
    #[serde(default)]
    forgotten: HashMap<String, String>,
    #[serde(default)]
    groups: Vec<Group>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyConfig {
    #[serde(default)]
    launcher: String,
    #[serde(default)]
    custom_launcher: String,
    auto_play: Option<bool>,
    stats: Option<bool>,
    aggressive: Option<bool>,
    window_title: Option<bool>,
}

#[derive(Deserialize)]
struct LegacyPlayerEntry {
    info: Option<Box<serde_json::value::RawValue>>,
    at: String,
}

#[derive(Deserialize)]
struct LegacyLaunch {
    #[serde(default)]
    uuid: String,
    at: String,
    #[serde(default)]
    pid: u32,
    #[serde(default)]
    seen: String,
    #[serde(default)]
    before: Vec<u32>,
}

#[derive(Deserialize)]
struct LegacyLogLine {
    #[serde(default)]
    at: i64,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct LegacyLogSession {
    #[serde(default)]
    started: i64,
    #[serde(default)]
    ended: i64,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    lines: Vec<LegacyLogLine>,
}

#[derive(Deserialize)]
struct LegacyLogClient {
    #[serde(default)]
    name: String,
    #[serde(default)]
    sessions: Vec<LegacyLogSession>,
}

pub(crate) async fn import(
    pool: &SqlitePool,
    secrets: &Arc<Secrets>,
    paths: &LegacyPaths,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let mut imported = Vec::new();

    if let Some(bytes) = read(&paths.vault)
        && let Ok(plain) = secrets.decrypt(&bytes)
        && let Ok(data) = serde_json::from_slice::<VaultData>(&plain)
    {
        import_vault(&mut tx, secrets, data).await?;
        imported.push(&paths.vault);
    }

    if let Some(bytes) = read(&paths.config)
        && let Ok(config) = serde_json::from_slice::<LegacyConfig>(&bytes)
    {
        import_config(&mut tx, config).await?;
        imported.push(&paths.config);
    }

    if let Some(bytes) = read(&paths.players)
        && let Ok(entries) = serde_json::from_slice::<HashMap<String, LegacyPlayerEntry>>(&bytes)
    {
        import_players(&mut tx, entries).await?;
        imported.push(&paths.players);
    }

    if let Some(bytes) = read(&paths.session)
        && let Ok(launches) = serde_json::from_slice::<Vec<LegacyLaunch>>(&bytes)
    {
        import_launches(&mut tx, launches).await?;
        imported.push(&paths.session);
    }

    if let Some(bytes) = read(&paths.logs)
        && let Ok(clients) = serde_json::from_slice::<HashMap<String, LegacyLogClient>>(&bytes)
    {
        import_logs(&mut tx, clients).await?;
        imported.push(&paths.logs);
    }

    tx.commit().await?;

    for path in imported {
        let backup = path.with_extension(backup_extension(path));
        let _ = fs::rename(path, backup);
    }
    Ok(())
}

fn read(path: &Path) -> Option<Vec<u8>> {
    fs::read(path).ok().filter(|bytes| !bytes.is_empty())
}

fn backup_extension(path: &Path) -> String {
    match path.extension().and_then(|value| value.to_str()) {
        Some(ext) => format!("{ext}.migrated"),
        None => "migrated".to_owned(),
    }
}

async fn import_vault(
    tx: &mut sqlx::SqliteConnection,
    secrets: &Arc<Secrets>,
    data: VaultData,
) -> Result<()> {
    for account in data.accounts.into_values() {
        let token = seal(secrets, &account.token)?;
        sqlx::query(
            "INSERT OR REPLACE INTO accounts (uuid, name, token, label, pinned, client, profile, \
             ram, min_graphics, fullscreen, discord_rpc, auto_enter, debug_mode, render_distance, \
             max_fps, animations, fast_render, minimal, expires, first_seen, last_seen, \
             last_launched, last_online) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
             ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
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
        .execute(&mut *tx)
        .await?;
    }
    for (uuid, token) in data.forgotten {
        sqlx::query("INSERT OR REPLACE INTO forgotten (uuid, token) VALUES (?1, ?2)")
            .bind(uuid)
            .bind(seal(secrets, &token)?)
            .execute(&mut *tx)
            .await?;
    }
    for (position, group) in data.groups.into_iter().enumerate() {
        sqlx::query(
            "INSERT OR REPLACE INTO account_groups (id, name, profile, pinned, sort_order, position) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&group.id)
        .bind(&group.name)
        .bind(&group.profile)
        .bind(i64::from(group.pinned))
        .bind(group.order)
        .bind(position as i64)
        .execute(&mut *tx)
        .await?;
        for (index, member) in group.members.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO group_members (group_id, account_uuid, position) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(&group.id)
            .bind(member)
            .bind(index as i64)
            .execute(&mut *tx)
            .await?;
        }
    }
    Ok(())
}

async fn import_config(tx: &mut sqlx::SqliteConnection, config: LegacyConfig) -> Result<()> {
    let launcher = LauncherKind::parse(&config.launcher)
        .filter(|kind| *kind != LauncherKind::Custom || !config.custom_launcher.is_empty())
        .unwrap_or_default();
    sqlx::query(
        "UPDATE settings SET launcher = ?1, custom_launcher = ?2, auto_play = ?3, stats = ?4, \
         aggressive = ?5, window_title = ?6 WHERE id = 1",
    )
    .bind(launcher.as_str())
    .bind(&config.custom_launcher)
    .bind(i64::from(config.auto_play.unwrap_or(true)))
    .bind(i64::from(config.stats.unwrap_or(true)))
    .bind(i64::from(config.aggressive.unwrap_or(false)))
    .bind(i64::from(config.window_title.unwrap_or(false)))
    .execute(&mut *tx)
    .await?;
    Ok(())
}

async fn import_players(
    tx: &mut sqlx::SqliteConnection,
    entries: HashMap<String, LegacyPlayerEntry>,
) -> Result<()> {
    for (name, entry) in entries {
        let Some(info) = entry.info else {
            continue;
        };
        let fetched = rfc3339_to_unix(&entry.at).unwrap_or_default();
        sqlx::query(
            "INSERT OR REPLACE INTO player_cache (name, info, fetched_at) VALUES (?1, ?2, ?3)",
        )
        .bind(name.to_lowercase())
        .bind(info.get())
        .bind(fetched)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

async fn import_launches(
    tx: &mut sqlx::SqliteConnection,
    launches: Vec<LegacyLaunch>,
) -> Result<()> {
    for (position, launch) in launches.into_iter().enumerate() {
        let seen = rfc3339_to_unix(&launch.seen);
        sqlx::query(
            "INSERT OR REPLACE INTO game_launches (uuid, position, launched_at, pid, seen_at, \
             before_pids) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&launch.uuid)
        .bind(position as i64)
        .bind(rfc3339_to_unix(&launch.at).unwrap_or_default())
        .bind(i64::from(launch.pid))
        .bind(seen)
        .bind(pack_pids(&launch.before))
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

async fn import_logs(
    tx: &mut sqlx::SqliteConnection,
    clients: HashMap<String, LegacyLogClient>,
) -> Result<()> {
    for (uuid, client) in clients {
        sqlx::query("INSERT OR REPLACE INTO log_clients (uuid, name) VALUES (?1, ?2)")
            .bind(&uuid)
            .bind(&client.name)
            .execute(&mut *tx)
            .await?;
        for session in client.sessions {
            let ended = if session.active || session.ended == 0 {
                session.lines.last().map_or(session.started, |line| line.at)
            } else {
                session.ended
            };
            let id = sqlx::query(
                "INSERT INTO log_sessions (uuid, started, ended, active) VALUES (?1, ?2, ?3, 0)",
            )
            .bind(&uuid)
            .bind(session.started)
            .bind(ended)
            .execute(&mut *tx)
            .await?
            .last_insert_rowid();
            for (seq, line) in session.lines.into_iter().enumerate() {
                sqlx::query(
                    "INSERT INTO log_lines (session_id, seq, at, text) VALUES (?1, ?2, ?3, ?4)",
                )
                .bind(id)
                .bind(seq as i64)
                .bind(line.at)
                .bind(line.text)
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    Ok(())
}

fn rfc3339_to_unix(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let bytes = value.as_bytes();
    let year: i64 = value.get(0..4)?.parse().ok()?;
    let month: i64 = value.get(5..7)?.parse().ok()?;
    let day: i64 = value.get(8..10)?.parse().ok()?;
    if bytes.get(10) != Some(&b'T') {
        return None;
    }
    let hour: i64 = value.get(11..13)?.parse().ok()?;
    let minute: i64 = value.get(14..16)?.parse().ok()?;
    let second: i64 = value.get(17..19)?.parse().ok()?;
    let mut offset = 0i64;
    if let Some(index) = value[19..].find(['+', '-']) {
        let sign = if value.as_bytes()[19 + index] == b'+' {
            1
        } else {
            -1
        };
        let rest = &value[20 + index..];
        let off_hour: i64 = rest.get(0..2)?.parse().ok()?;
        let off_min: i64 = rest
            .get(3..5)
            .and_then(|part| part.parse().ok())
            .unwrap_or(0);
        offset = sign * (off_hour * 3600 + off_min * 60);
    }
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::rfc3339_to_unix;

    #[test]
    fn parses_utc_timestamp() {
        assert_eq!(rfc3339_to_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339_to_unix("2021-01-01T00:00:00Z"), Some(1_609_459_200));
    }

    #[test]
    fn applies_zone_offset() {
        assert_eq!(
            rfc3339_to_unix("2021-01-01T03:00:00+03:00"),
            Some(1_609_459_200)
        );
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(rfc3339_to_unix(""), None);
    }
}
