use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::command::detached;

#[cfg(windows)]
const JAVA_EXE: &str = "java.exe";
#[cfg(not(windows))]
const JAVA_EXE: &str = "java";
const MIN_MAJOR: u32 = 11;

pub async fn resolve_java(cristalix: &Path) -> PathBuf {
    for candidate in candidates(cristalix) {
        if !tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            continue;
        }
        if java_major(&candidate).await >= MIN_MAJOR {
            return candidate;
        }
    }
    PathBuf::from("java")
}

fn candidates(cristalix: &Path) -> Vec<PathBuf> {
    let mut out = bundled_runtimes(&cristalix.join("updates"));
    out.push(cristalix.join("runtime").join("bin").join(JAVA_EXE));
    if let Some(home) = std::env::var_os("JAVA_HOME").filter(|home| !home.is_empty()) {
        out.push(PathBuf::from(home).join("bin").join(JAVA_EXE));
    }
    if let Some(found) = on_path() {
        out.push(found);
    }
    #[cfg(target_os = "linux")]
    out.extend([
        PathBuf::from("/usr/lib/jvm/default-java/bin/java"),
        PathBuf::from("/usr/lib/jvm/default/bin/java"),
    ]);
    out
}

fn bundled_runtimes(updates: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(updates) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("jre"))
        .map(|entry| entry.path().join("bin").join(JAVA_EXE))
        .filter(|path| path.exists())
        .collect();
    found.sort_unstable();
    found
}

fn on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(JAVA_EXE))
        .find(|candidate| candidate.is_file())
}

async fn java_major(java: &Path) -> u32 {
    let mut command = detached(java);
    command
        .arg("-version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let Ok(output) = command.output().await else {
        return 0;
    };
    if !output.status.success() {
        return 0;
    }
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    parse_java_major(&text)
}

fn parse_java_major(text: &str) -> u32 {
    const MARKER: &str = "version \"";
    let Some(start) = text.find(MARKER) else {
        return 0;
    };
    let rest = &text[start + MARKER.len()..];
    let version = rest.split('"').next().unwrap_or_default();
    let version = version.strip_prefix("1.").unwrap_or(version);
    version
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0u32, |acc, digit| {
            acc.saturating_mul(10)
                .saturating_add(u32::from(digit - b'0'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_and_modern_version_strings() {
        assert_eq!(
            parse_java_major("java version \"1.8.0_381\"\nJava(TM) SE"),
            8
        );
        assert_eq!(
            parse_java_major("openjdk version \"17.0.8\" 2023-07-18"),
            17
        );
        assert_eq!(parse_java_major("openjdk version \"21\" 2023-09-19"), 21);
        assert_eq!(parse_java_major("no version here"), 0);
    }

    #[test]
    fn bundled_runtimes_are_sorted_and_must_exist() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["zulu-jre17", "a-jre21", "client"] {
            std::fs::create_dir_all(dir.path().join(name).join("bin")).unwrap();
        }
        std::fs::write(
            dir.path().join("zulu-jre17").join("bin").join(JAVA_EXE),
            b"",
        )
        .unwrap();
        std::fs::write(dir.path().join("a-jre21").join("bin").join(JAVA_EXE), b"").unwrap();
        std::fs::write(dir.path().join("client").join("bin").join(JAVA_EXE), b"").unwrap();
        let found = bundled_runtimes(dir.path());
        assert_eq!(found.len(), 2);
        assert!(found[0].starts_with(dir.path().join("a-jre21")));
    }
}
