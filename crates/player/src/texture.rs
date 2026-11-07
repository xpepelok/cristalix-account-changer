use std::collections::HashMap;
use std::time::{Duration, Instant};

use bytes::Bytes;
use parking_lot::Mutex;
use reqwest::header::{ORIGIN, REFERER};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use crate::error::{PlayerError, TextureError};
use crate::uuid::looks_like_uuid;

pub const TEXTURE_TTL: Duration = Duration::from_secs(300);

const TEXTURE_BASE: &str = "https://webdata.c7x.dev/textures";
const TEXTURE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextureKind {
    Skin,
    Cape,
}

impl TextureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skin => "skin",
            Self::Cape => "cape",
        }
    }
}

struct Cached {
    png: Option<Bytes>,
    stored_at: Instant,
}

pub struct TextureClient {
    http: Client,
    cache: Mutex<HashMap<(TextureKind, String), Cached>>,
}

impl TextureClient {
    pub fn new() -> Result<Self, PlayerError> {
        Ok(Self {
            http: Client::builder().timeout(TEXTURE_TIMEOUT).build()?,
            cache: Mutex::default(),
        })
    }

    pub async fn fetch(&self, kind: TextureKind, uuid: &str) -> Result<Bytes, TextureError> {
        if !looks_like_uuid(uuid) {
            return Err(TextureError::InvalidUuid);
        }
        let key = (kind, uuid.to_ascii_lowercase());
        if let Some(hit) = self.lookup(&key) {
            return hit.ok_or(TextureError::NotFound);
        }

        let response = self
            .http
            .get(format!("{TEXTURE_BASE}/{}/{uuid}", kind.as_str()))
            .header(ORIGIN, "https://cristalix.gg")
            .header(REFERER, "https://cristalix.gg/")
            .send()
            .await?;
        let png = if response.status() == StatusCode::OK {
            Some(response.bytes().await?)
        } else {
            None
        };
        self.remember(key, png.clone());
        png.ok_or(TextureError::NotFound)
    }

    pub fn forget(&self, uuid: &str) {
        self.cache
            .lock()
            .retain(|(_, cached_uuid), _| !cached_uuid.eq_ignore_ascii_case(uuid));
    }

    fn lookup(&self, key: &(TextureKind, String)) -> Option<Option<Bytes>> {
        let cache = self.cache.lock();
        let hit = cache.get(key)?;
        (hit.stored_at.elapsed() < TEXTURE_TTL).then(|| hit.png.clone())
    }

    fn remember(&self, key: (TextureKind, String), png: Option<Bytes>) {
        let mut cache = self.cache.lock();
        cache.retain(|_, cached| cached.stored_at.elapsed() < TEXTURE_TTL);
        cache.insert(
            key,
            Cached {
                png,
                stored_at: Instant::now(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_invalid_uuid_without_network() {
        let client = TextureClient::new().unwrap();
        assert!(matches!(
            client.fetch(TextureKind::Skin, "../etc/passwd").await,
            Err(TextureError::InvalidUuid)
        ));
    }

    #[tokio::test]
    async fn serves_cached_hits_and_misses() {
        let client = TextureClient::new().unwrap();
        let uuid = "3e7839e2-a269-11ed-b93f-1cb72cb014ae";
        client.remember(
            (TextureKind::Skin, uuid.into()),
            Some(Bytes::from_static(b"png")),
        );
        client.remember((TextureKind::Cape, uuid.into()), None);
        assert_eq!(
            client
                .fetch(TextureKind::Skin, &uuid.to_ascii_uppercase())
                .await
                .unwrap(),
            Bytes::from_static(b"png")
        );
        assert!(matches!(
            client.fetch(TextureKind::Cape, uuid).await,
            Err(TextureError::NotFound)
        ));
        client.forget(&uuid.to_ascii_uppercase());
        assert!(client.lookup(&(TextureKind::Skin, uuid.into())).is_none());
    }

    #[tokio::test]
    #[ignore = "hits webdata.c7x.dev"]
    async fn live_skin_is_png() {
        let client = TextureClient::new().unwrap();
        let png = client
            .fetch(TextureKind::Skin, "3e7839e2-a269-11ed-b93f-1cb72cb014ae")
            .await
            .unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }
}
