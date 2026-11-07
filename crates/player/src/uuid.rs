pub fn looks_like_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
    use super::looks_like_uuid;

    #[test]
    fn accepts_canonical_uuids() {
        assert!(looks_like_uuid("3e7839e2-a269-11ed-b93f-1cb72cb014ae"));
        assert!(looks_like_uuid("3E7839E2-A269-11ED-B93F-1CB72CB014AE"));
    }

    #[test]
    fn rejects_malformed_values() {
        for value in [
            "",
            "3e7839e2a26911edb93f1cb72cb014ae",
            "3e7839e2-a269-11ed-b93f-1cb72cb014a",
            "3e7839e2-a269-11ed-b93f-1cb72cb014ag",
            "3e7839e2+a269-11ed-b93f-1cb72cb014ae",
            "../../../../etc/passwd/aaaaaaaaaaaaaaaa",
            "3e7839e2-a269-11ed-b93f-1cb72cb014ж",
        ] {
            assert!(!looks_like_uuid(value), "{value}");
        }
    }
}
