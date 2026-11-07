use serde::{Deserialize, Serialize};

use crate::version::{CURRENT_VERSION, compare_versions, extract_version};

pub const ASSET_NAME: &str = if cfg!(windows) {
    "AccountChanger.exe"
} else {
    "AccountChanger-linux-x86_64"
};

pub fn asset_matches(name: &str) -> bool {
    name == ASSET_NAME
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub notes: String,
    pub name: String,
    pub published_at: String,
    #[serde(skip)]
    pub(crate) asset_url: Option<String>,
}

impl UpdateInfo {
    pub(crate) fn unknown() -> Self {
        Self {
            current: CURRENT_VERSION.to_owned(),
            latest: CURRENT_VERSION.to_owned(),
            ..Self::default()
        }
    }

    pub(crate) fn from_release(release: Release) -> Option<Self> {
        let latest = extract_version(&release.tag_name)?.to_owned();
        let asset_url = release
            .assets
            .into_iter()
            .find(|asset| asset_matches(&asset.name))
            .map(|asset| asset.browser_download_url);
        Some(Self {
            current: CURRENT_VERSION.to_owned(),
            available: asset_url.is_some() && compare_versions(&latest, CURRENT_VERSION).is_gt(),
            notes: release.body.trim().to_owned(),
            name: release.name.trim().to_owned(),
            published_at: release.published_at,
            latest,
            asset_url,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Release {
    #[serde(deserialize_with = "nullable")]
    tag_name: String,
    #[serde(deserialize_with = "nullable")]
    name: String,
    #[serde(deserialize_with = "nullable")]
    body: String,
    #[serde(deserialize_with = "nullable")]
    published_at: String,
    #[serde(deserialize_with = "nullable")]
    assets: Vec<Asset>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Asset {
    #[serde(deserialize_with = "nullable")]
    name: String,
    #[serde(deserialize_with = "nullable")]
    browser_download_url: String,
}

fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE_ASSETS: [&str; 6] = [
        "AccountChanger.exe",
        "AccountChanger-linux-x86_64",
        "AccountChanger-linux-x86_64-webkit40",
        "AccountChanger-linux-x86_64-nogui",
        "checksums.txt",
        "Source code (zip)",
    ];

    #[test]
    fn exactly_one_asset_matches_this_platform() {
        let picked: Vec<_> = RELEASE_ASSETS
            .into_iter()
            .filter(|name| asset_matches(name))
            .collect();
        assert_eq!(picked, [ASSET_NAME]);
    }

    #[test]
    fn rejects_lookalikes() {
        for name in [
            "accountchanger.exe",
            "AccountChanger.exe.sig",
            "AccountChanger-linux-x86_64.exe",
            "AccountChanger-linux-x86_64-nogui",
            "AccountChanger-linux-x86_64.tar.gz",
            "AccountChanger-darwin-arm64",
        ] {
            assert!(!asset_matches(name), "{name}");
        }
    }

    fn release(tag: &str, assets: &[&str]) -> Release {
        let json = serde_json::json!({
            "tag_name": tag,
            "name": "  AccountChanger 9.9.9 ",
            "body": "\n- notes\n",
            "published_at": "2026-11-01T10:00:00Z",
            "assets": assets
                .iter()
                .map(|name| serde_json::json!({"name": name, "browser_download_url": format!("https://example.invalid/{name}")}))
                .collect::<Vec<_>>(),
        });
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn newer_release_with_asset_is_available() {
        let info = UpdateInfo::from_release(release("v9.9.9", &RELEASE_ASSETS)).unwrap();
        assert!(info.available);
        assert_eq!(info.latest, "9.9.9");
        assert_eq!(info.current, CURRENT_VERSION);
        assert_eq!(info.notes, "- notes");
        assert_eq!(info.name, "AccountChanger 9.9.9");
        assert_eq!(info.published_at, "2026-11-01T10:00:00Z");
        assert_eq!(
            info.asset_url.as_deref(),
            Some(format!("https://example.invalid/{ASSET_NAME}").as_str())
        );
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["publishedAt"], "2026-11-01T10:00:00Z");
        assert!(json.get("assetUrl").is_none());
    }

    #[test]
    fn missing_asset_or_older_tag_is_not_available() {
        assert!(
            !UpdateInfo::from_release(release("v9.9.9", &["checksums.txt"]))
                .unwrap()
                .available
        );
        assert!(
            !UpdateInfo::from_release(release("v0.0.1", &RELEASE_ASSETS))
                .unwrap()
                .available
        );
        assert!(
            !UpdateInfo::from_release(release(CURRENT_VERSION, &RELEASE_ASSETS))
                .unwrap()
                .available
        );
        assert!(UpdateInfo::from_release(release("nightly", &RELEASE_ASSETS)).is_none());
    }

    #[test]
    fn tolerates_nulls() {
        let parsed: Release =
            serde_json::from_str(r#"{"tag_name":"v1.0.0","name":null,"body":null,"assets":null}"#)
                .unwrap();
        let info = UpdateInfo::from_release(parsed).unwrap();
        assert_eq!(info.latest, "1.0.0");
        assert!(!info.available);
    }
}
