use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use parking_lot::Mutex;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};

use crate::error::PlayerError;
use crate::info::{PlayerInfo, SearchEntry, names_match};
use crate::time::parse_rfc3339;

pub const CACHE_TTL: Duration = Duration::from_secs(60);
pub const CACHE_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
pub const FLUSH_INTERVAL: Duration = Duration::from_secs(30);
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(2 * 60);

const NEGATIVE_TTL: Duration = Duration::from_secs(30);
const STALE_RETRY: Duration = Duration::from_secs(25);
const MIN_GAP: Duration = Duration::from_millis(600);
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(90);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(10 * 60);
const SERVER_ERROR_COOLDOWN: Duration = Duration::from_secs(25);
const RETRY_DELAY: Duration = Duration::from_millis(700);
const ATTEMPTS: usize = 3;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

const SEARCH_URL: &str = "https://top.cristalix.gg/api/web/players/search";
const SEARCH_LIMIT: &str = "8";

const BROWSER_HEADERS: [(&str, &str); 10] = [
    (
        "user-agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    ),
    ("accept", "application/json, text/plain, */*"),
    ("accept-language", "ru-RU,ru;q=0.9,en;q=0.8"),
    ("referer", "https://top.cristalix.gg/"),
    (
        "sec-ch-ua",
        "\"Chromium\";v=\"133\", \"Not(A:Brand\";v=\"24\", \"Google Chrome\";v=\"133\"",
    ),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", "\"Windows\""),
    ("sec-fetch-dest", "empty"),
    ("sec-fetch-mode", "cors"),
    ("sec-fetch-site", "same-origin"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub info: PlayerInfo,
    pub fetched_at: SystemTime,
}

#[derive(Clone)]
struct Slot {
    info: Option<Arc<PlayerInfo>>,
    fetched_at: SystemTime,
}

impl Slot {
    fn is_fresh(&self, now: SystemTime) -> bool {
        let ttl = if self.info.is_some() {
            CACHE_TTL
        } else {
            NEGATIVE_TTL
        };
        now.duration_since(self.fetched_at).unwrap_or_default() < ttl
    }
}

#[derive(Default)]
struct Cache {
    slots: HashMap<String, Slot>,
    dirty: bool,
}

#[derive(Default)]
struct Gate {
    cooldown_until: Option<Instant>,
    next_slot: Option<Instant>,
}

enum Attempt {
    Found(Box<PlayerInfo>),
    Missing,
    Retry,
}

pub struct PlayerService {
    http: Client,
    cache: Mutex<Cache>,
    gate: Mutex<Gate>,
}

impl PlayerService {
    pub fn new() -> Result<Self, PlayerError> {
        let headers = BROWSER_HEADERS
            .iter()
            .map(|(name, value)| {
                (
                    HeaderName::from_static(name),
                    HeaderValue::from_static(value),
                )
            })
            .collect::<HeaderMap>();
        let http = Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            http,
            cache: Mutex::default(),
            gate: Mutex::default(),
        })
    }

    pub fn cached(&self, name: &str) -> Option<Arc<PlayerInfo>> {
        self.cache
            .lock()
            .slots
            .get(&cache_key(name))
            .and_then(|slot| slot.info.clone())
    }

    pub fn cached_staff(&self, name: &str) -> Option<String> {
        self.cached(name).map(|info| info.staff.clone())
    }

    pub fn in_cooldown(&self) -> bool {
        self.gate
            .lock()
            .cooldown_until
            .is_some_and(|until| Instant::now() < until)
    }

    pub fn invalidate(&self, name: &str) {
        if !self.in_cooldown() {
            self.cache.lock().slots.remove(&cache_key(name));
        }
    }

    pub async fn fetch(&self, name: &str) -> Option<Arc<PlayerInfo>> {
        let key = cache_key(name);
        let previous = self.cache.lock().slots.get(&key).cloned();
        if let Some(slot) = &previous
            && slot.is_fresh(SystemTime::now())
        {
            return slot.info.clone();
        }
        let stale = previous.and_then(|slot| slot.info);
        if self.in_cooldown() {
            return stale;
        }

        match (self.fetch_remote(name).await, stale) {
            (None, Some(stale)) => {
                let retry_at = SystemTime::now() - (CACHE_TTL - STALE_RETRY);
                self.store(key, Some(stale.clone()), retry_at);
                Some(stale)
            }
            (fresh, _) => {
                let fresh = fresh.map(Arc::new);
                self.store(key, fresh.clone(), SystemTime::now());
                fresh
            }
        }
    }

    pub async fn refresh_loop<F>(&self, names: F)
    where
        F: Fn() -> Vec<String>,
    {
        loop {
            tokio::time::sleep(REFRESH_INTERVAL).await;
            if self.in_cooldown() {
                continue;
            }
            for name in names() {
                if self.in_cooldown() {
                    break;
                }
                self.fetch(&name).await;
            }
        }
    }

    pub fn import(&self, entries: impl IntoIterator<Item = CacheEntry>) {
        let now = SystemTime::now();
        let mut cache = self.cache.lock();
        for CacheEntry {
            key,
            mut info,
            fetched_at,
        } in entries
        {
            if now.duration_since(fetched_at).unwrap_or_default() > CACHE_MAX_AGE {
                continue;
            }
            info.forget_live_state();
            cache.slots.insert(
                key,
                Slot {
                    info: Some(Arc::new(info)),
                    fetched_at,
                },
            );
        }
    }

    pub fn export(&self) -> Vec<CacheEntry> {
        collect_entries(&self.cache.lock())
    }

    pub fn take_dirty(&self) -> Option<Vec<CacheEntry>> {
        let mut cache = self.cache.lock();
        if !cache.dirty {
            return None;
        }
        cache.dirty = false;
        Some(collect_entries(&cache))
    }

    fn store(&self, key: String, info: Option<Arc<PlayerInfo>>, fetched_at: SystemTime) {
        let mut cache = self.cache.lock();
        cache.dirty |= info.is_some();
        cache.slots.insert(key, Slot { info, fetched_at });
    }

    fn start_cooldown(&self, duration: Duration) {
        let until = Instant::now() + duration;
        let mut gate = self.gate.lock();
        if gate.cooldown_until.is_none_or(|current| until > current) {
            gate.cooldown_until = Some(until);
        }
    }

    fn reserve_turn(&self) -> Duration {
        let now = Instant::now();
        let mut gate = self.gate.lock();
        let wait = gate
            .next_slot
            .map_or(Duration::ZERO, |next| next.saturating_duration_since(now));
        gate.next_slot = Some(now + wait + MIN_GAP);
        wait
    }

    async fn fetch_remote(&self, name: &str) -> Option<PlayerInfo> {
        for attempt in 0..ATTEMPTS {
            match self.try_fetch(name).await {
                Attempt::Found(info) => return Some(*info),
                Attempt::Missing => return None,
                Attempt::Retry => {}
            }
            if self.in_cooldown() {
                return None;
            }
            if attempt + 1 < ATTEMPTS {
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
        None
    }

    async fn try_fetch(&self, name: &str) -> Attempt {
        let Ok(url) = Url::parse_with_params(SEARCH_URL, [("q", name), ("limit", SEARCH_LIMIT)])
        else {
            return Attempt::Retry;
        };

        let wait = self.reserve_turn();
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        let Ok(response) = self.http.get(url).send().await else {
            return Attempt::Retry;
        };

        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            let header = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok());
            self.start_cooldown(retry_after(header));
            return Attempt::Missing;
        }
        if status.is_server_error() {
            self.start_cooldown(SERVER_ERROR_COOLDOWN);
            return Attempt::Missing;
        }
        if status != StatusCode::OK {
            return Attempt::Retry;
        }
        let Ok(body) = response.bytes().await else {
            return Attempt::Retry;
        };
        let Ok(entries) = serde_json::from_slice::<Vec<SearchEntry>>(&body) else {
            return Attempt::Retry;
        };
        entries
            .into_iter()
            .find(|entry| names_match(entry.name(), name))
            .map_or(Attempt::Missing, |entry| {
                Attempt::Found(Box::new(entry.into_info()))
            })
    }
}

fn cache_key(name: &str) -> String {
    name.to_lowercase()
}

fn collect_entries(cache: &Cache) -> Vec<CacheEntry> {
    cache
        .slots
        .iter()
        .filter_map(|(key, slot)| {
            slot.info.as_ref().map(|info| CacheEntry {
                key: key.clone(),
                info: PlayerInfo::clone(info),
                fetched_at: slot.fetched_at,
            })
        })
        .collect()
}

fn retry_after(header: Option<&str>) -> Duration {
    header
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|seconds| *seconds > 0)
        .map_or(RATE_LIMIT_COOLDOWN, |seconds| {
            Duration::from_secs(seconds.unsigned_abs()).min(MAX_RETRY_AFTER)
        })
}

#[derive(Deserialize)]
struct LegacyEntry {
    info: Option<PlayerInfo>,
    at: String,
}

pub fn parse_legacy_cache(json: &[u8]) -> Result<Vec<CacheEntry>, PlayerError> {
    let stored: HashMap<String, LegacyEntry> = serde_json::from_slice(json)?;
    Ok(stored
        .into_iter()
        .filter_map(|(key, entry)| {
            Some(CacheEntry {
                key,
                info: entry.info?,
                fetched_at: parse_rfc3339(&entry.at)?,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info::Presence;

    fn entry(key: &str, age: Duration) -> CacheEntry {
        CacheEntry {
            key: key.into(),
            info: PlayerInfo {
                name: key.into(),
                staff: "PLAYER".into(),
                online: Presence::Online,
                realm: "PRS-1".into(),
                ..PlayerInfo::default()
            },
            fetched_at: SystemTime::now() - age,
        }
    }

    #[test]
    fn retry_after_parsing() {
        assert_eq!(retry_after(Some(" 12 ")), Duration::from_secs(12));
        assert_eq!(retry_after(Some("0")), RATE_LIMIT_COOLDOWN);
        assert_eq!(retry_after(Some("soon")), RATE_LIMIT_COOLDOWN);
        assert_eq!(retry_after(None), RATE_LIMIT_COOLDOWN);
        assert_eq!(retry_after(Some("99999")), MAX_RETRY_AFTER);
    }

    #[test]
    fn import_drops_expired_and_live_state() {
        let service = PlayerService::new().unwrap();
        service.import([
            entry("fresh", Duration::from_secs(10)),
            entry("old", CACHE_MAX_AGE + Duration::from_secs(1)),
        ]);
        let fresh = service.cached("FRESH").unwrap();
        assert_eq!(fresh.online, Presence::Unknown);
        assert_eq!(fresh.realm, "");
        assert_eq!(service.cached_staff("fresh").as_deref(), Some("PLAYER"));
        assert!(service.cached("old").is_none());
        assert!(service.take_dirty().is_none());
        assert_eq!(service.export().len(), 1);
    }

    #[tokio::test]
    async fn fresh_cache_short_circuits_network() {
        let service = PlayerService::new().unwrap();
        service.import([entry("cached", Duration::from_secs(5))]);
        assert_eq!(service.fetch("Cached").await.unwrap().name, "cached");
    }

    #[test]
    fn store_marks_dirty_only_for_hits() {
        let service = PlayerService::new().unwrap();
        service.store("ghost".into(), None, SystemTime::now());
        assert!(service.take_dirty().is_none());
        service.store(
            "real".into(),
            Some(Arc::new(PlayerInfo::default())),
            SystemTime::now(),
        );
        assert_eq!(service.take_dirty().unwrap().len(), 1);
        assert!(service.take_dirty().is_none());
    }

    #[test]
    fn negative_entries_expire_faster() {
        let now = SystemTime::now();
        let negative = Slot {
            info: None,
            fetched_at: now - Duration::from_secs(31),
        };
        let positive = Slot {
            info: Some(Arc::new(PlayerInfo::default())),
            fetched_at: now - Duration::from_secs(31),
        };
        assert!(!negative.is_fresh(now));
        assert!(positive.is_fresh(now));
    }

    #[test]
    fn turns_are_spaced() {
        let service = PlayerService::new().unwrap();
        assert_eq!(service.reserve_turn(), Duration::ZERO);
        let second = service.reserve_turn();
        let third = service.reserve_turn();
        assert!(second > MIN_GAP - Duration::from_millis(50) && second <= MIN_GAP);
        assert!(third > second + MIN_GAP - Duration::from_millis(50));
    }

    #[test]
    fn invalidate_respects_cooldown() {
        let service = PlayerService::new().unwrap();
        service.import([entry("a", Duration::from_secs(1))]);
        service.start_cooldown(Duration::from_secs(60));
        service.invalidate("a");
        assert!(service.cached("a").is_some());
    }

    #[test]
    fn legacy_players_json_is_migrated() {
        let json = br#"{
            "xpepelok": {"info": {"name": "xpepelok", "staff": "sr_dev", "online": "hidden", "likes": 1}, "at": "2026-11-05T03:09:14.5+03:00"},
            "ghost": {"info": null, "at": "2026-11-05T03:09:14Z"}
        }"#;
        let entries = parse_legacy_cache(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "xpepelok");
        assert_eq!(entries[0].info.likes, 1);
        assert_eq!(
            entries[0].fetched_at,
            std::time::UNIX_EPOCH + Duration::new(1_793_837_354, 500_000_000)
        );
    }

    #[tokio::test]
    #[ignore = "hits top.cristalix.gg"]
    async fn live_search_returns_player() {
        let service = PlayerService::new().unwrap();
        let info = service.fetch("xpepelok").await.expect("player info");
        assert_eq!(info.name, "xpepelok");
        assert!(!info.staff.is_empty());
    }
}
