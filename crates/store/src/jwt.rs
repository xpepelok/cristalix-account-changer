use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::Deserialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Claims {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub iat: i64,
    #[serde(default)]
    pub exp: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JwtError {
    Malformed,
    Payload,
    MissingUuid,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Malformed => "malformed token",
            Self::Payload => "invalid token payload",
            Self::MissingUuid => "token has no uuid",
        };
        f.write_str(message)
    }
}

impl std::error::Error for JwtError {}

pub fn parse(token: &str) -> Result<Claims, JwtError> {
    let payload = token.split('.').nth(1).ok_or(JwtError::Malformed)?;
    if payload.is_empty() && token.split('.').count() < 2 {
        return Err(JwtError::Malformed);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| STANDARD.decode(payload))
        .map_err(|_| JwtError::Payload)?;
    let mut claims: Claims = serde_json::from_slice(&bytes).map_err(|_| JwtError::Payload)?;
    if claims.uuid.is_empty() {
        return Err(JwtError::MissingUuid);
    }
    if let Some(canonical) = crate::uuid::canonical_uuid(&claims.uuid) {
        claims.uuid = canonical;
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::{JwtError, parse};
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    fn token(payload: &str) -> String {
        format!("header.{}.sig", URL_SAFE_NO_PAD.encode(payload.as_bytes()))
    }

    #[test]
    fn parses_claims() {
        let claims = parse(&token(r#"{"uuid":"abc","exp":42,"purpose":"auth"}"#)).unwrap();
        assert_eq!(claims.uuid, "abc");
        assert_eq!(claims.exp, 42);
        assert_eq!(claims.purpose, "auth");
    }

    #[test]
    fn rejects_malformed() {
        assert_eq!(parse("single"), Err(JwtError::Malformed));
    }

    #[test]
    fn rejects_missing_uuid() {
        assert_eq!(parse(&token(r#"{"exp":1}"#)), Err(JwtError::MissingUuid));
    }
}
