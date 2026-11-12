use std::collections::BTreeSet;

use ac_api::{ApiError, ApiResult, EmojiAtlas, EmojiSet, msg};
use serde_json::Value;

const INDEX_URL: &str = "https://webdata.c7x.dev/client/emojis.json";
const PROVIDED: u32 = 0xE010;
const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Default, PartialEq, Eq)]
struct Plan {
    atlases: Vec<(u32, u32, String)>,
    singles: Vec<(u32, String)>,
    aliases: Vec<(u32, u32)>,
}

fn code_of(entry: &Value) -> Option<u32> {
    let id = u32::try_from(entry.get("id")?.as_u64()?).ok()?;
    PROVIDED.checked_add(id)
}

fn entries<'a>(index: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    index
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn url_of(entry: &Value) -> Option<String> {
    entry
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("https://"))
        .map(str::to_owned)
}

fn plan(index: &Value, codes: &[u32]) -> Plan {
    let mut wanted: BTreeSet<u32> = codes.iter().copied().collect();
    let mut plan = Plan::default();
    for entry in entries(index, "animatedList") {
        let Some(code) = code_of(entry).filter(|code| wanted.contains(code)) else {
            continue;
        };
        let frame = entry
            .get("frames")
            .and_then(Value::as_array)
            .and_then(|frames| frames.first())
            .and_then(Value::as_str)
            .and_then(|frame| frame.chars().next())
            .map(u32::from);
        if let Some(frame) = frame {
            plan.aliases.push((code, frame));
            wanted.insert(frame);
        }
    }
    for entry in entries(index, "atlasList") {
        let (Some(start), Some(count), Some(url)) = (
            code_of(entry),
            entry
                .get("totalIcons")
                .and_then(Value::as_u64)
                .and_then(|count| u32::try_from(count).ok()),
            url_of(entry),
        ) else {
            continue;
        };
        if wanted
            .range(start..start.saturating_add(count))
            .next()
            .is_some()
        {
            plan.atlases.push((start, count, url));
        }
    }
    for entry in entries(index, "emojiList") {
        if let (Some(code), Some(url)) = (code_of(entry), url_of(entry))
            && wanted.contains(&code)
        {
            plan.singles.push((code, url));
        }
    }
    plan
}

async fn bytes(http: &reqwest::Client, url: &str) -> ApiResult<Vec<u8>> {
    let response = http
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|err| ApiError::new(err.to_string()))?;
    let body = response
        .bytes()
        .await
        .map_err(|err| ApiError::new(err.to_string()))?;
    if body.len() > MAX_BYTES {
        return Err(ApiError::fixed(msg::GENERIC));
    }
    Ok(body.to_vec())
}

pub(crate) async fn fetch(http: &reqwest::Client, codes: &[u32]) -> ApiResult<EmojiSet> {
    if codes.is_empty() {
        return Ok(EmojiSet::default());
    }
    let index = bytes(http, INDEX_URL).await?;
    let index: Value = serde_json::from_slice(&index).map_err(|_| ApiError::fixed(msg::GENERIC))?;
    let plan = plan(&index, codes);
    let mut set = EmojiSet {
        aliases: plan.aliases,
        ..EmojiSet::default()
    };
    for (start, count, url) in plan.atlases {
        if let Ok(png) = bytes(http, &url).await {
            set.atlases.push(EmojiAtlas { start, count, png });
        }
    }
    for (code, url) in plan.singles {
        if let Ok(png) = bytes(http, &url).await {
            set.singles.push((code, png));
        }
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Value {
        serde_json::from_str(
            r#"{"emojiList":[{"id":5163,"name":"tg","url":"https://x/tg.png"}],
            "atlasList":[{"id":42,"totalIcons":1024,"url":"https://x/general.png"},
            {"id":1066,"totalIcons":1024,"url":"https://x/subs.png"}],
            "animatedList":[{"id":4138,"frames":["",""]}]}"#,
        )
        .expect("index json")
    }

    #[test]
    fn only_needed_atlas_is_planned() {
        let plan = plan(&index(), &[0xE120]);
        assert_eq!(
            plan.atlases,
            [(0xE03A, 1024, "https://x/general.png".to_owned())]
        );
        assert!(plan.singles.is_empty());
    }

    #[test]
    fn single_and_animated_are_resolved() {
        let plan = plan(&index(), &[PROVIDED + 5163, PROVIDED + 4138]);
        assert_eq!(
            plan.singles,
            [(PROVIDED + 5163, "https://x/tg.png".to_owned())]
        );
        assert_eq!(plan.aliases, [(PROVIDED + 4138, 0xE41A)]);
        assert_eq!(plan.atlases.len(), 1);
        assert_eq!(plan.atlases[0].0, 0xE03A);
    }

    #[test]
    fn unknown_codes_plan_nothing() {
        assert_eq!(plan(&index(), &[0xE005]), Plan::default());
    }
}
