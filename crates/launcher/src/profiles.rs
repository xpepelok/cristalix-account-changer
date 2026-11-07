use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

pub use ac_api::{MINIMAL_PROFILE, PROFILE_FILES, PROFILE_NAME_MAX_CHARS};

use crate::options::{MINIMAL_OPTIONS, minimal_optionsof};

pub fn sanitize_profile_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '.' => '_',
            other => other,
        })
        .take(PROFILE_NAME_MAX_CHARS)
        .collect();
    cleaned.trim().to_owned()
}

pub fn list_profiles(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    out.sort_unstable();
    out
}

pub fn save_profile_from_client(dir: &Path, name: &str, client_dir: &Path) -> io::Result<u32> {
    let dest = dir.join(name);
    fs::create_dir_all(&dest)?;
    let copied = PROFILE_FILES
        .iter()
        .filter(|file| {
            fs::read(client_dir.join(file))
                .and_then(|data| fs::write(dest.join(file), data))
                .is_ok()
        })
        .count();
    Ok(copied as u32)
}

pub fn read_profile(dir: &Path, name: &str) -> BTreeMap<String, String> {
    let base = dir.join(name);
    PROFILE_FILES
        .iter()
        .map(|file| {
            let content = fs::read(base.join(file))
                .map(|data| String::from_utf8_lossy(&data).into_owned())
                .unwrap_or_default();
            ((*file).to_owned(), content)
        })
        .collect()
}

pub fn write_profile(dir: &Path, name: &str, files: &BTreeMap<String, String>) -> io::Result<()> {
    let dest = dir.join(name);
    fs::create_dir_all(&dest)?;
    for file in PROFILE_FILES {
        if let Some(content) = files.get(file) {
            fs::write(dest.join(file), content)?;
        }
    }
    Ok(())
}

pub fn delete_profile(dir: &Path, name: &str) -> io::Result<()> {
    match fs::remove_dir_all(dir.join(name)) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

pub(crate) fn apply_profile(dir: &Path, name: &str, client_dir: &Path) {
    let base = dir.join(name);
    for file in PROFILE_FILES {
        if let Ok(data) = fs::read(base.join(file)) {
            let _ = fs::write(client_dir.join(file), data);
        }
    }
}

pub fn minimal_profile_content() -> BTreeMap<String, String> {
    let options: BTreeMap<&str, &str> = MINIMAL_OPTIONS.iter().copied().collect();
    BTreeMap::from([
        ("options.txt".to_owned(), profile_text(&options)),
        (
            "optionsof.txt".to_owned(),
            profile_text(&minimal_optionsof()),
        ),
        ("binds.json".to_owned(), String::new()),
        ("voicechat.json".to_owned(), String::new()),
    ])
}

fn profile_text(values: &BTreeMap<&str, &str>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{key}:{value}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_reserved_characters_and_truncates() {
        assert_eq!(
            sanitize_profile_name("  a/b\\c:d*e?f\"g<h>i|j.k  "),
            "a_b_c_d_e_f_g_h_i_j_k"
        );
        assert_eq!(sanitize_profile_name("   "), "");
        let long = "я".repeat(50);
        assert_eq!(sanitize_profile_name(&long).chars().count(), 40);
        let padded = format!("{} x", "ж".repeat(39));
        assert_eq!(sanitize_profile_name(&padded), "ж".repeat(39));
        assert_eq!(sanitize_profile_name(MINIMAL_PROFILE), MINIMAL_PROFILE);
    }

    #[test]
    fn save_read_write_delete_round_trip() {
        let root = tempfile::tempdir().unwrap();
        let profiles = root.path().join("profiles");
        let client = root.path().join("client");
        fs::create_dir_all(&client).unwrap();
        fs::write(client.join("options.txt"), "fov:90").unwrap();
        fs::write(client.join("binds.json"), "{}").unwrap();

        assert_eq!(
            save_profile_from_client(&profiles, "PvP", &client).unwrap(),
            2
        );
        assert_eq!(list_profiles(&profiles), ["PvP"]);
        let content = read_profile(&profiles, "PvP");
        assert_eq!(content.len(), 4);
        assert_eq!(content["options.txt"], "fov:90");
        assert_eq!(content["voicechat.json"], "");

        let edits = BTreeMap::from([
            ("options.txt".to_owned(), "fov:70".to_owned()),
            ("evil.txt".to_owned(), "x".to_owned()),
        ]);
        write_profile(&profiles, "PvP", &edits).unwrap();
        assert!(!profiles.join("PvP").join("evil.txt").exists());

        let target = root.path().join("target");
        fs::create_dir_all(&target).unwrap();
        apply_profile(&profiles, "PvP", &target);
        assert_eq!(
            fs::read_to_string(target.join("options.txt")).unwrap(),
            "fov:70"
        );
        assert!(!target.join("optionsof.txt").exists());

        delete_profile(&profiles, "PvP").unwrap();
        delete_profile(&profiles, "PvP").unwrap();
        assert!(list_profiles(&profiles).is_empty());
    }

    #[test]
    fn minimal_profile_is_sorted_key_value_text() {
        let content = minimal_profile_content();
        let options = &content["options.txt"];
        assert!(options.starts_with("ao:0\nbiomeBlendRadius:0\n"));
        assert!(options.ends_with("useVbo:true\n"));
        assert!(content["optionsof.txt"].contains("ofAnimatedWater:0\n"));
        assert!(content["optionsof.txt"].contains("ofFastRender:true\n"));
        assert_eq!(content["binds.json"], "");
    }
}
