use std::cmp::Ordering;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn extract_version(tag: &str) -> Option<&str> {
    let start = tag.find(|c: char| c.is_ascii_digit())?;
    let rest = &tag[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    Some(rest[..end].trim_matches('.')).filter(|version| !version.is_empty())
}

pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let mut left = left.split('.').map(leading_number);
    let mut right = right.split('.').map(leading_number);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (a, b) => match a.unwrap_or(0).cmp(&b.unwrap_or(0)) {
                Ordering::Equal => continue,
                other => return other,
            },
        }
    }
}

fn leading_number(part: &str) -> u64 {
    let digits = part
        .find(|c: char| !c.is_ascii_digit())
        .map_or(part, |end| &part[..end]);
    digits.parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_versions_from_tags() {
        assert_eq!(extract_version("v0.4.0"), Some("0.4.0"));
        assert_eq!(extract_version("release-1.2.3-beta"), Some("1.2.3"));
        assert_eq!(extract_version("0.3.6"), Some("0.3.6"));
        assert_eq!(extract_version("v1.2."), Some("1.2"));
        assert_eq!(extract_version("v.1"), Some("1"));
        assert_eq!(extract_version("latest"), None);
        assert_eq!(extract_version(""), None);
    }

    #[test]
    fn compares_numerically() {
        assert_eq!(compare_versions("0.4.0", "0.3.6"), Ordering::Greater);
        assert_eq!(compare_versions("0.3.10", "0.3.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.3.6", "0.4.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.1", "1.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.2rc", "1.2"), Ordering::Equal);
        assert_eq!(compare_versions("x.1", "0.1"), Ordering::Equal);
    }
}
