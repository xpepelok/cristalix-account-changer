use std::path::PathBuf;
use std::sync::Arc;

use ac_launcher::{Account, Change, Host, LaunchConfig, LaunchRecord};
use ac_player::PlayerService;
use ac_store::{Store, jwt};
use tokio::sync::mpsc;

use crate::convert::{game_launch, launch_account, launcher_kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Signal {
    Queue,
    GroupLaunch,
    Logs,
}

pub(crate) struct StoreHost {
    pub store: Arc<Store>,
    pub players: Arc<PlayerService>,
    pub signals: mpsc::UnboundedSender<Signal>,
}

pub(crate) fn is_staff_rank(staff: Option<&str>) -> bool {
    staff.is_some_and(|staff| !staff.is_empty() && !staff.eq_ignore_ascii_case("PLAYER"))
}

impl Host for StoreHost {
    fn account(&self, uuid: &str) -> Option<Account> {
        self.store.get(uuid).map(launch_account)
    }

    fn accounts(&self) -> Vec<Account> {
        self.store.list().into_iter().map(launch_account).collect()
    }

    fn config(&self) -> LaunchConfig {
        let settings = self.store.settings();
        LaunchConfig {
            launcher: launcher_kind(settings.launcher),
            custom_launcher: PathBuf::from(settings.custom_launcher),
            aggressive: settings.aggressive,
            auto_play: settings.auto_play,
            window_title: settings.window_title,
        }
    }

    fn is_staff(&self, name: &str) -> bool {
        is_staff_rank(self.players.cached_staff(name).as_deref())
    }

    fn notify(&self, change: Change) {
        let signal = match change {
            Change::Queue => Signal::Queue,
            Change::GroupLaunch => Signal::GroupLaunch,
        };
        let _ = self.signals.send(signal);
    }

    async fn capture_token(&self, name: &str, token: &str) {
        if let Ok(claims) = jwt::parse(token) {
            let _ = self.store.upsert_token(name, token, &claims).await;
        }
    }

    async fn set_client(&self, uuid: &str, client: &str) {
        let _ = self.store.set_client(uuid, client).await;
    }

    async fn mark_launched(&self, uuid: &str) {
        let _ = self.store.mark_launched(uuid).await;
    }

    async fn touch_online(&self, uuids: Vec<String>) {
        let _ = self.store.touch_online(&uuids).await;
    }

    async fn save_launches(&self, records: Vec<LaunchRecord>) {
        let launches: Vec<_> = records.into_iter().map(game_launch).collect();
        let _ = self.store.save_launches(&launches).await;
    }

    async fn log_begin(&self, uuid: &str, name: &str) -> Option<i64> {
        let session = self.store.begin_log_session(uuid, name).await.ok();
        let _ = self.signals.send(Signal::Logs);
        session
    }

    async fn log_append(&self, session: i64, lines: Vec<String>) {
        let _ = self.store.append_log_lines(session, &lines).await;
    }

    async fn log_finish(&self, session: i64) {
        let _ = self.store.finish_log_session(session).await;
        let _ = self.signals.send(Signal::Logs);
    }
}
