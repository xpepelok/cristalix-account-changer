use std::env;
use std::path::PathBuf;

fn absolute_var(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

pub(crate) fn home() -> PathBuf {
    env::home_dir().unwrap_or_default()
}

pub(crate) fn data_home() -> PathBuf {
    absolute_var("XDG_DATA_HOME").unwrap_or_else(|| home().join(".local").join("share"))
}

pub(crate) fn config_home() -> PathBuf {
    absolute_var("XDG_CONFIG_HOME").unwrap_or_else(|| home().join(".config"))
}

pub(crate) fn quote_exec(path: &str) -> String {
    if !path.contains([' ', '"', '\'', '\\', '`', '$']) {
        return path.to_owned();
    }
    let mut out = String::with_capacity(path.len() + 4);
    out.push('"');
    for ch in path.chars() {
        if matches!(ch, '\\' | '"' | '`' | '$') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::quote_exec;

    #[test]
    fn plain_paths_are_left_alone() {
        assert_eq!(quote_exec("/usr/bin/ac"), "/usr/bin/ac");
    }

    #[test]
    fn special_characters_are_escaped() {
        assert_eq!(
            quote_exec("/opt/my app/a$b\"c"),
            "\"/opt/my app/a\\$b\\\"c\""
        );
    }
}
