use std::sync::Arc;

use ac_api::{BackendEvent, ImportStatus};

use crate::core::Inner;
use crate::import::Task;

impl Inner {
    pub(crate) fn emit_import(&self) {
        self.events
            .emit(BackendEvent::Import(self.import.progress()));
    }

    pub(crate) fn spawn_import(self: &Arc<Self>, tasks: Vec<Task>) {
        let inner = Arc::clone(self);
        self.emit_import();
        self.handle.clone().spawn(async move {
            inner.run_import(tasks).await;
            inner.import.finish();
            inner.emit_import();
        });
    }

    pub(crate) fn spawn_refresh(self: &Arc<Self>, tasks: Vec<Task>) {
        let inner = Arc::clone(self);
        self.emit_import();
        self.handle.clone().spawn(async move {
            inner.run_refresh(tasks).await;
            inner.import.finish();
            inner.emit_import();
        });
    }

    pub(crate) fn set_item(
        &self,
        index: usize,
        status: ImportStatus,
        text: &str,
        name: Option<&str>,
    ) {
        self.import.set_item(index, status, text, name);
        self.emit_import();
    }
}

#[cfg(not(windows))]
impl Inner {
    async fn run_import(self: &Arc<Self>, tasks: Vec<Task>) {
        for task in tasks {
            if self.import.cancelled() {
                self.import.mark_rest_cancelled();
                break;
            }
            self.set_item(
                task.index,
                ImportStatus::Err,
                ac_api::msg::NO_UI_AUTOMATION,
                None,
            );
        }
    }

    async fn run_refresh(self: &Arc<Self>, tasks: Vec<Task>) {
        self.run_import(tasks).await;
    }
}

#[cfg(windows)]
mod windows {
    use std::sync::Arc;
    use std::time::Duration;

    use ac_api::{ImportStatus, msg};
    use ac_launcher::{
        STAFF_LAUNCHER_URL, ScriptOutcome, annul_account, apply_account, current_client, kill_tree,
        process_tree, spawn_launcher, uia_login, uia_logout,
    };
    use tokio::process::Child;
    use tokio::time::Instant;

    use crate::convert::{is_expired, unix_now};
    use crate::core::Inner;
    use crate::import::Task;

    const WINDOW_WAIT: Duration = Duration::from_secs(120);
    const WINDOW_POLL: Duration = Duration::from_millis(400);
    const TOKEN_POLL: Duration = Duration::from_millis(150);
    const AUTH_RETRIES: u32 = 3;
    const PLAY_TIMEOUT: u32 = 25;
    const LOGIN_TIMEOUT: u32 = 90;
    const LOGOUT_TIMEOUT: u32 = 25;

    pub(crate) fn valid_login(account: &ac_store::Account, now: i64) -> Option<String> {
        if account.name.is_empty() || account.token.is_empty() || is_expired(account.expires, now) {
            return None;
        }
        Some(account.name.to_lowercase())
    }

    pub(crate) fn saved_valid_logins(inner: &Inner) -> std::collections::HashSet<String> {
        let now = unix_now();
        inner
            .store
            .list()
            .iter()
            .filter_map(|account| valid_login(account, now))
            .collect()
    }

    fn launcher_window_pid(root: u32) -> u32 {
        process_tree(root)
            .into_iter()
            .find(|pid| ac_platform::window_for_pid(*pid).is_some())
            .unwrap_or(0)
    }

    fn retryable(error: &str) -> bool {
        !matches!(
            error,
            msg::WRONG_CREDENTIALS
                | msg::CREDENTIALS_REQUIRED
                | msg::CANCELED
                | msg::AUTH_LOGIN_UNKNOWN
                | msg::NO_UI_AUTOMATION
        )
    }

    struct Launcher {
        child: Child,
    }

    impl Launcher {
        fn pid(&self) -> u32 {
            self.child.id().unwrap_or(0)
        }
    }

    impl Inner {
        async fn ensure_import_launcher(
            &self,
            prepare: Option<&(dyn Fn() + Send + Sync)>,
        ) -> Result<Launcher, String> {
            match prepare {
                Some(prepare) => prepare(),
                None => {
                    let _ = annul_account(&self.paths.launcher_cfg);
                }
            }
            ac_platform::clear_stale_locks(&self.paths.cristalix);
            self.downloader
                .ensure(&self.paths.staff_launcher_exe, STAFF_LAUNCHER_URL)
                .await
                .map_err(|err| msg::launcher_prepare_failed(&err.to_string()))?;
            let child = spawn_launcher(&self.paths.staff_launcher_exe, None)
                .map_err(|err| msg::launcher_start_failed(&err.to_string()))?;
            let launcher = Launcher { child };
            let root = launcher.pid();
            let deadline = Instant::now() + WINDOW_WAIT;
            while Instant::now() < deadline {
                if self.import.cancelled() {
                    return Err(msg::CANCELED.to_owned());
                }
                if launcher_window_pid(root) != 0 {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    return Ok(launcher);
                }
                tokio::time::sleep(WINDOW_POLL).await;
            }
            kill_tree(root).await;
            Err(msg::LAUNCHER_WINDOW_MISSING.to_owned())
        }

        async fn wait_vault_token(
            &self,
            before: &std::collections::HashSet<String>,
            timeout: Duration,
        ) -> Option<(String, String)> {
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                if self.import.cancelled() {
                    return None;
                }
                let now = unix_now();
                for account in self.store.list() {
                    if let Some(key) = valid_login(&account, now)
                        && !before.contains(&key)
                    {
                        return Some((account.uuid, account.name));
                    }
                }
                tokio::time::sleep(TOKEN_POLL).await;
            }
            None
        }

        async fn wait_refreshed_token(
            &self,
            uuid: &str,
            old_token: &str,
            timeout: Duration,
        ) -> bool {
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                if self.import.cancelled() {
                    return false;
                }
                if let Some(account) = self.store.get(uuid)
                    && !account.token.is_empty()
                    && account.token != old_token
                {
                    return true;
                }
                tokio::time::sleep(TOKEN_POLL).await;
            }
            false
        }

        fn login_error(outcome: &ScriptOutcome) -> Option<&'static str> {
            match outcome.code {
                1 => Some(msg::LOGIN_FORM_MISSING),
                2 => Some(msg::LAUNCHER_FOCUS_ADMIN),
                3 => {
                    let lowered = outcome.output.to_lowercase();
                    if outcome.output.contains("SendWait") || lowered.contains("denied") {
                        Some(msg::LAUNCHER_INPUT_ADMIN)
                    } else {
                        Some(msg::CREDENTIALS_INPUT_FAILED)
                    }
                }
                4 => Some(msg::WRONG_CREDENTIALS),
                _ => None,
            }
        }

        async fn import_one(
            &self,
            launcher: &Launcher,
            login: &str,
            password: &str,
        ) -> Result<String, String> {
            let login = login.trim();
            if login.is_empty() || password.is_empty() {
                return Err(msg::CREDENTIALS_REQUIRED.to_owned());
            }
            let before = saved_valid_logins(self);
            let mut backoff = Duration::from_secs(2);
            let mut auth_retries = 0;
            loop {
                let outcome = uia_login(login, password, LOGIN_TIMEOUT).await;
                if let Some(error) = Self::login_error(&outcome) {
                    return Err(error.to_owned());
                }
                if outcome.code == 5 {
                    if auth_retries < AUTH_RETRIES && !self.import.cancelled() {
                        auth_retries += 1;
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    return Err(msg::AUTH_LOGIN_UNKNOWN.to_owned());
                }
                break;
            }
            let (uuid, name) = match self
                .wait_vault_token(&before, Duration::from_secs(10))
                .await
            {
                Some(found) => found,
                None => {
                    let pid = launcher.pid();
                    let games = Arc::clone(self.tracker.games());
                    tokio::spawn(async move {
                        ac_launcher::click_play_for_pid(&games, pid, PLAY_TIMEOUT).await;
                    });
                    self.wait_vault_token(&before, Duration::from_secs(35))
                        .await
                        .ok_or_else(|| msg::LOGIN_TIMEOUT.to_owned())?
                }
            };
            self.store.set_password(&uuid, password).await.ok();
            Ok(name)
        }

        pub(crate) async fn run_import(self: &Arc<Self>, tasks: Vec<Task>) {
            let mut launcher: Option<Launcher> = None;
            let mut reuse_ready = false;
            for (position, task) in tasks.iter().enumerate() {
                if self.import.cancelled() {
                    self.teardown(launcher.take()).await;
                    self.import.mark_rest_cancelled();
                    self.emit_import();
                    return;
                }
                if !reuse_ready || launcher.is_none() {
                    self.teardown(launcher.take()).await;
                    match self.ensure_import_launcher(None).await {
                        Ok(fresh) => launcher = Some(fresh),
                        Err(error) => {
                            self.set_item(task.index, ImportStatus::Err, &error, None);
                            continue;
                        }
                    }
                }
                reuse_ready = false;
                self.set_item(task.index, ImportStatus::Working, "вход…", None);
                let active = launcher.as_ref().expect("launcher present");
                let mut result = self.import_one(active, &task.login, &task.secret).await;
                if result.as_ref().err().is_some_and(|error| retryable(error))
                    && !self.import.cancelled()
                {
                    self.teardown(launcher.take()).await;
                    if let Ok(fresh) = self.ensure_import_launcher(None).await {
                        launcher = Some(fresh);
                        self.set_item(task.index, ImportStatus::Working, "повтор…", None);
                        let active = launcher.as_ref().expect("launcher present");
                        result = self.import_one(active, &task.login, &task.secret).await;
                    }
                }
                if self.import.cancelled() {
                    if let Ok(name) = &result {
                        self.set_item(
                            task.index,
                            ImportStatus::Ok,
                            &format!("✓ {name}"),
                            Some(name),
                        );
                    }
                    self.teardown(launcher.take()).await;
                    self.import.mark_rest_cancelled();
                    self.emit_import();
                    return;
                }
                match &result {
                    Ok(name) => self.set_item(
                        task.index,
                        ImportStatus::Ok,
                        &format!("✓ {name}"),
                        Some(name),
                    ),
                    Err(error) => self.set_item(task.index, ImportStatus::Err, error, None),
                }
                let last = position + 1 == tasks.len();
                if result.is_ok() && !last {
                    reuse_ready = uia_logout(LOGOUT_TIMEOUT).await.code == 0;
                }
                if !reuse_ready {
                    self.teardown(launcher.take()).await;
                }
            }
            self.teardown(launcher.take()).await;
        }

        pub(crate) async fn run_refresh(self: &Arc<Self>, tasks: Vec<Task>) {
            for task in tasks {
                if self.import.cancelled() {
                    self.import.mark_rest_cancelled();
                    self.emit_import();
                    return;
                }
                self.set_item(
                    task.index,
                    ImportStatus::Working,
                    msg::REFRESH_WORKING,
                    None,
                );
                let result = self.refresh_token(&task.uuid).await;
                if self.import.cancelled() {
                    if let Ok((name, true)) = &result {
                        self.set_item(
                            task.index,
                            ImportStatus::Ok,
                            &format!("✓ {name}"),
                            Some(name),
                        );
                    }
                    self.import.mark_rest_cancelled();
                    self.emit_import();
                    return;
                }
                match result {
                    Err(error) => self.set_item(task.index, ImportStatus::Err, &error, None),
                    Ok((name, false)) => self.set_item(
                        task.index,
                        ImportStatus::Skip,
                        msg::REFRESH_TOO_EARLY,
                        Some(&name),
                    ),
                    Ok((name, true)) => self.set_item(
                        task.index,
                        ImportStatus::Ok,
                        &format!("✓ {name}"),
                        Some(&name),
                    ),
                }
            }
            let _ = annul_account(&self.paths.launcher_cfg);
        }

        async fn refresh_token(&self, uuid: &str) -> Result<(String, bool), String> {
            let Some(account) = self
                .store
                .get(uuid)
                .filter(|account| !account.name.is_empty() && !account.token.is_empty())
            else {
                return Err(msg::NO_SAVED_TOKEN.to_owned());
            };
            let name = account.name.clone();
            let old_token = account.token.clone();
            let expired = is_expired(account.expires, unix_now());
            let client = if account.client.is_empty() {
                current_client(&self.paths.launcher_cfg)
            } else {
                account.client.clone()
            };
            let mut opts = launch_opts_from(&account);
            opts.auto_enter = false;
            let cfg = self.paths.launcher_cfg.clone();
            let launcher = self
                .ensure_import_launcher(Some(&|| {
                    let _ = apply_account(&cfg, &name, &old_token, &client, &opts);
                }))
                .await?;
            if self.import.cancelled() {
                self.teardown(Some(launcher)).await;
                return Err(msg::CANCELED.to_owned());
            }
            let pid = launcher.pid();
            let games = Arc::clone(self.tracker.games());
            tokio::spawn(async move {
                ac_launcher::click_play_for_pid(&games, pid, PLAY_TIMEOUT).await;
            });
            let wait = if expired {
                Duration::from_secs(3 * 60)
            } else {
                Duration::from_secs(35)
            };
            let refreshed = self.wait_refreshed_token(uuid, &old_token, wait).await;
            self.teardown(Some(launcher)).await;
            if refreshed {
                Ok((name, true))
            } else if expired {
                Err(msg::TOKEN_EXPIRED_NEEDS_PASSWORD.to_owned())
            } else {
                Ok((name, false))
            }
        }

        async fn teardown(&self, launcher: Option<Launcher>) {
            if let Some(launcher) = launcher {
                kill_tree(launcher.pid()).await;
            }
        }
    }

    fn launch_opts_from(account: &ac_store::Account) -> ac_launcher::LaunchOptions {
        let ram = if (account.minimal || account.profile == ac_launcher::MINIMAL_PROFILE)
            && account.ram < ac_launcher::MIN_RAM
        {
            ac_launcher::MIN_RAM
        } else {
            account.ram
        };
        ac_launcher::LaunchOptions {
            ram,
            min_graphics: account.min_graphics,
            fullscreen: account.fullscreen,
            discord_rpc: account.discord_rpc,
            auto_enter: account.auto_enter,
            debug_mode: account.debug_mode,
        }
    }
}
