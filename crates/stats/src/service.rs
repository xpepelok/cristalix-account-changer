use std::path::Path;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::time::MissedTickBehavior;

use crate::error::StatsError;
use crate::install_id::install_id;

pub const PING_INTERVAL: Duration = Duration::from_secs(5 * 60);

const PING_URL: &str = "https://stats.xpepelok.me/ping";
const INFO_URL: &str = "https://stats.xpepelok.me/stats";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const PING_TIMEOUT: Duration = Duration::from_secs(8);
const INFO_TIMEOUT: Duration = Duration::from_secs(6);
const FRESH_FOR: Duration = Duration::from_secs(45);
const FAIL_BACKOFF: Duration = Duration::from_secs(3 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnlineCounts {
    pub online: u64,
    pub active: u64,
    pub total: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum Lookup {
    Cached(Option<OnlineCounts>),
    Refresh,
}

#[derive(Default)]
struct Snapshot {
    counts: Option<OnlineCounts>,
    fetched_at: Option<Instant>,
    retry_at: Option<Instant>,
}

impl Snapshot {
    fn lookup(&self, now: Instant) -> Lookup {
        let fresh = self
            .fetched_at
            .is_some_and(|at| now.duration_since(at) < FRESH_FOR);
        let backing_off = self.retry_at.is_some_and(|at| now < at);
        match (self.counts, fresh, backing_off) {
            (Some(counts), true, _) => Lookup::Cached(Some(counts)),
            (counts, _, true) => Lookup::Cached(counts),
            _ => Lookup::Refresh,
        }
    }
}

pub struct StatsService {
    http: Client,
    snapshot: Mutex<Snapshot>,
}

impl StatsService {
    pub fn new() -> Result<Self, StatsError> {
        Ok(Self {
            http: Client::builder().build()?,
            snapshot: Mutex::default(),
        })
    }

    pub async fn ping_loop<F>(&self, data_dir: &Path, enabled: F)
    where
        F: Fn() -> bool,
    {
        let mut ticker = tokio::time::interval(PING_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if enabled() {
                self.ping(data_dir).await;
            }
        }
    }

    pub async fn ping(&self, data_dir: &Path) {
        let Some(id) = install_id(data_dir) else {
            return;
        };
        if let Ok(response) = self
            .http
            .get(format!("{PING_URL}?id={id}&v={VERSION}"))
            .timeout(PING_TIMEOUT)
            .send()
            .await
        {
            let _ = response.bytes().await;
        }
    }

    pub async fn fetch(&self) -> Option<OnlineCounts> {
        if let Lookup::Cached(cached) = self.snapshot.lock().lookup(Instant::now()) {
            return cached;
        }
        let fetched = self.fetch_remote().await;
        let mut snapshot = self.snapshot.lock();
        match fetched {
            Some(counts) => {
                *snapshot = Snapshot {
                    counts: Some(counts),
                    fetched_at: Some(Instant::now()),
                    retry_at: None,
                };
                Some(counts)
            }
            None => {
                snapshot.retry_at = Some(Instant::now() + FAIL_BACKOFF);
                snapshot.counts
            }
        }
    }

    async fn fetch_remote(&self) -> Option<OnlineCounts> {
        let response = self
            .http
            .get(INFO_URL)
            .timeout(INFO_TIMEOUT)
            .send()
            .await
            .ok()?;
        if response.status() != StatusCode::OK {
            return None;
        }
        serde_json::from_slice(&response.bytes().await.ok()?).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUNTS: OnlineCounts = OnlineCounts {
        online: 3,
        active: 10,
        total: 42,
    };

    #[test]
    fn fresh_snapshot_is_served_from_cache() {
        let now = Instant::now();
        let snapshot = Snapshot {
            counts: Some(COUNTS),
            fetched_at: Some(now),
            retry_at: None,
        };
        assert_eq!(
            snapshot.lookup(now + Duration::from_secs(10)),
            Lookup::Cached(Some(COUNTS))
        );
        assert_eq!(snapshot.lookup(now + FRESH_FOR), Lookup::Refresh);
    }

    #[test]
    fn backoff_returns_last_known_value() {
        let now = Instant::now();
        let failed = Snapshot {
            counts: None,
            fetched_at: None,
            retry_at: Some(now + FAIL_BACKOFF),
        };
        assert_eq!(failed.lookup(now), Lookup::Cached(None));
        assert_eq!(failed.lookup(now + FAIL_BACKOFF), Lookup::Refresh);
        let stale = Snapshot {
            counts: Some(COUNTS),
            fetched_at: Some(now),
            retry_at: Some(now + FRESH_FOR + FAIL_BACKOFF),
        };
        assert_eq!(
            stale.lookup(now + FRESH_FOR + Duration::from_secs(1)),
            Lookup::Cached(Some(COUNTS))
        );
        assert_eq!(Snapshot::default().lookup(now), Lookup::Refresh);
    }

    #[test]
    fn counts_json_shape() {
        let parsed: OnlineCounts =
            serde_json::from_str(r#"{"online":3,"active":10,"total":42}"#).unwrap();
        assert_eq!(parsed, COUNTS);
    }

    #[tokio::test]
    #[ignore = "hits stats.xpepelok.me"]
    async fn live_fetch() {
        let service = StatsService::new().unwrap();
        assert!(service.fetch().await.is_some());
    }
}
