use std::collections::HashSet;
use std::time::Duration;

use tokio::sync::Notify;

use crate::games::DEFAULT_GAME_TITLE;
use crate::host::Host;
use crate::tracker::GameTracker;

const TITLE_INTERVAL: Duration = Duration::from_secs(3);
const KICK_SETTLE: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct Titles {
    kick: Notify,
}

impl Titles {
    pub fn kick(&self) {
        self.kick.notify_one();
    }

    pub async fn run<H: Host>(&self, tracker: &GameTracker, host: &H) {
        let mut renamed: HashSet<u32> = HashSet::new();
        loop {
            tokio::select! {
                () = tokio::time::sleep(TITLE_INTERVAL) => {}
                () = self.kick.notified() => tokio::time::sleep(KICK_SETTLE).await,
            }
            let presence = tracker.resolve();
            host.touch_online(presence.running.keys().cloned().collect())
                .await;
            if !host.config().window_title {
                if renamed.is_empty() {
                    continue;
                }
                let live: HashSet<u32> = presence.running.values().copied().collect();
                let restore: Vec<u32> = renamed.drain().filter(|pid| live.contains(pid)).collect();
                for pid in restore {
                    set_title(pid, DEFAULT_GAME_TITLE.to_owned()).await;
                }
                continue;
            }
            for (uuid, pid) in &presence.running {
                let Some(account) = host
                    .account(uuid)
                    .filter(|account| !account.name.is_empty())
                else {
                    continue;
                };
                tracker.games().allow_titles([account.name.as_str()]);
                if set_title(*pid, account.name).await {
                    renamed.insert(*pid);
                }
            }
        }
    }
}

async fn set_title(pid: u32, title: String) -> bool {
    tokio::task::spawn_blocking(move || ac_platform::set_window_title_for_pid(pid, &title))
        .await
        .unwrap_or(false)
}
