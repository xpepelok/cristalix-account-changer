use std::collections::HashMap;
use std::sync::OnceLock;

use ac_api::{ApiError, ApiResult, CabinetStatus, PunishmentKind, Segment, Violation, msg};
use parking_lot::Mutex;
use serde_json::Value;

const FETCH_URL: &str = "https://cristalix.gg/api/cabinet/bans/fetch";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const NO_RECORD: &str = "no-ban-found";
const SEPARATORS: [char; 2] = ['┃', '|'];
const PLAYER_URL: &str = "https://top.cristalix.gg/api/web/players";

type Issuer = (String, Vec<Segment>);

fn issuers() -> &'static Mutex<HashMap<String, Issuer>> {
    static ISSUERS: OnceLock<Mutex<HashMap<String, Issuer>>> = OnceLock::new();
    ISSUERS.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn issuer(http: &reqwest::Client, uuid: &str) -> Issuer {
    if uuid.is_empty() {
        return Issuer::default();
    }
    if let Some(issuer) = issuers().lock().get(uuid) {
        return issuer.clone();
    }
    let response = http
        .get(format!("{PLAYER_URL}/{uuid}"))
        .header("user-agent", UA)
        .header("referer", "https://top.cristalix.gg/")
        .send()
        .await;
    let Ok(response) = response else {
        return Issuer::default();
    };
    let Ok(body) = response.text().await else {
        return Issuer::default();
    };
    let issuer = parse_issuer(&body);
    if !issuer.0.is_empty() {
        issuers().lock().insert(uuid.to_owned(), issuer.clone());
    }
    issuer
}

fn parse_issuer(body: &str) -> Issuer {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Issuer::default();
    };
    let display = value
        .get("social")
        .and_then(|social| social.get("formattedName"))
        .and_then(Value::as_str)
        .map(|formatted| {
            let mut segments: Vec<Segment> = ac_player::formatted_segments(formatted)
                .into_iter()
                .map(|(text, color)| Segment {
                    text: text.replace(SEPARATORS, ""),
                    color,
                })
                .filter(|segment| !segment.text.trim().is_empty())
                .collect();
            for index in 1..segments.len() {
                let joined = segments[index - 1].text.ends_with(' ')
                    || segments[index].text.starts_with(' ');
                if !joined {
                    segments[index - 1].text.push(' ');
                }
            }
            if let Some(last) = segments.last_mut() {
                last.text = last.text.trim_end().to_owned();
            }
            segments
        })
        .unwrap_or_default();
    (string_field(&value, "name"), display)
}

pub(crate) async fn fetch_status(
    http: &reqwest::Client,
    session: &str,
) -> ApiResult<CabinetStatus> {
    let mut ban = fetch_one(http, session, PunishmentKind::Ban).await?;
    let mut mute = fetch_one(http, session, PunishmentKind::Mute).await?;
    for violation in [ban.as_mut(), mute.as_mut()].into_iter().flatten() {
        (violation.issued_by_name, violation.issued_by_display) =
            issuer(http, &violation.issued_by).await;
    }
    Ok(CabinetStatus { ban, mute })
}

async fn fetch_one(
    http: &reqwest::Client,
    session: &str,
    kind: PunishmentKind,
) -> ApiResult<Option<Violation>> {
    let response = http
        .post(FETCH_URL)
        .header("cookie", format!("PHPSESSID={session}"))
        .header("user-agent", UA)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("x-requested-with", "XMLHttpRequest")
        .header("accept", "application/json, text/plain, */*")
        .body(format!("type={}", kind.as_str()))
        .send()
        .await
        .map_err(|err| ApiError::new(err.to_string()))?;
    let text = response
        .text()
        .await
        .map_err(|err| ApiError::new(err.to_string()))?;
    parse_violation(kind, &text)
}

fn parse_violation(kind: PunishmentKind, body: &str) -> ApiResult<Option<Violation>> {
    let value: Value = serde_json::from_str(body).map_err(|_| ApiError::fixed(msg::GENERIC))?;
    if value.get("success").and_then(Value::as_bool) == Some(true) {
        let result = value.get("result").unwrap_or(&Value::Null);
        let violation = result.get("violation").unwrap_or(&Value::Null);
        return Ok(Some(Violation {
            kind,
            reason: string_field(violation, "reason"),
            issued_by: string_field(violation, "whoBanned"),
            issued_by_name: String::new(),
            issued_by_display: Vec::new(),
            remaining: int_field(violation, "banTime"),
            permanent: violation
                .get("permanent")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            expires_at: int_field(violation, "unbanTime"),
            price: int_field(result, "price"),
        }));
    }
    match value.get("error").and_then(Value::as_str) {
        Some(NO_RECORD) | None => Ok(None),
        Some("unauthorized") | Some("not-authorized") | Some("no-session") => {
            Err(ApiError::fixed(msg::CABINET_SESSION))
        }
        Some(other) => Err(ApiError::new(other.to_owned())),
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn int_field(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|field| field.as_i64().or_else(|| field.as_f64().map(|v| v as i64)))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issuer_display_drops_separator() {
        let (name, display) = parse_issuer(
            r#"{"name":"xpepelok","social":{"formattedName":"§9Sr.Dev §8┃ §axpepelok"}}"#,
        );
        assert_eq!(name, "xpepelok");
        let text: String = display
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        assert_eq!(text, "Sr.Dev xpepelok");
        assert_eq!(display.len(), 2);
    }

    #[test]
    fn parses_no_record() {
        let out = parse_violation(
            PunishmentKind::Ban,
            r#"{"success":false,"error":"no-ban-found"}"#,
        );
        assert_eq!(out.unwrap(), None);
    }

    #[test]
    fn parses_active_ban() {
        let body = r#"{"success":true,"result":{"violation":{"reason":"3.5","playerName":"uuid-a","whoBanned":"uuid-b","banTime":431987911,"permanent":false,"unbanTime":1794619178302},"price":2000}}"#;
        let out = parse_violation(PunishmentKind::Ban, body).unwrap().unwrap();
        assert_eq!(out.reason, "3.5");
        assert_eq!(out.issued_by, "uuid-b");
        assert_eq!(out.remaining, 431987911);
        assert!(!out.permanent);
        assert_eq!(out.expires_at, 1794619178302);
        assert_eq!(out.price, 2000);
        assert_eq!(out.kind, PunishmentKind::Ban);
    }

    #[test]
    fn parses_permanent_mute() {
        let body = r#"{"success":true,"result":{"violation":{"reason":"spam","whoBanned":"staff","banTime":10,"permanent":true,"unbanTime":0},"price":0}}"#;
        let out = parse_violation(PunishmentKind::Mute, body)
            .unwrap()
            .unwrap();
        assert!(out.permanent);
        assert_eq!(out.kind, PunishmentKind::Mute);
    }

    #[test]
    fn dead_session_errors() {
        let out = parse_violation(
            PunishmentKind::Ban,
            r#"{"success":false,"error":"unauthorized"}"#,
        );
        assert!(out.is_err());
    }
}
