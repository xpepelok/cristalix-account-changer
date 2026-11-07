use std::fs;
use std::path::Path;

pub fn list_clients(updates: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(updates) else {
        return Vec::new();
    };
    let mut clients: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| entry.path().join("minecraft.jar").exists())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    clients.sort_unstable();
    clients
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_directories_with_a_client_jar_are_listed() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["Minigames", "Anarchy", "Empty"] {
            fs::create_dir_all(dir.path().join(name)).unwrap();
        }
        fs::write(dir.path().join("Minigames").join("minecraft.jar"), b"").unwrap();
        fs::write(dir.path().join("Anarchy").join("minecraft.jar"), b"").unwrap();
        fs::write(dir.path().join("minecraft.jar"), b"").unwrap();
        assert_eq!(list_clients(dir.path()), ["Anarchy", "Minigames"]);
        assert!(list_clients(&dir.path().join("missing")).is_empty());
    }
}
