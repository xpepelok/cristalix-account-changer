use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    pub cmdline: String,
}

impl ProcInfo {
    pub fn is_java(&self) -> bool {
        if self.comm.to_lowercase().contains("java") {
            return true;
        }
        let first = self.cmdline.split(' ').next().unwrap_or_default();
        Path::new(first)
            .file_name()
            .is_some_and(|name| name.to_string_lossy().to_lowercase().contains("java"))
    }

    pub fn is_game(&self) -> bool {
        if self.cmdline.contains("Cristalix.jar") || self.cmdline.contains("CristalixLauncher") {
            return false;
        }
        self.cmdline.contains("minecraft.jar") || self.cmdline.contains("net.minecraft.client")
    }
}

pub(crate) fn scan_procs(with_cmdline: bool) -> Vec<ProcInfo> {
    ac_platform::processes()
        .into_iter()
        .map(|info| ProcInfo {
            pid: info.pid,
            ppid: info.ppid,
            cmdline: with_cmdline
                .then(|| ac_platform::command_line(info.pid))
                .flatten()
                .unwrap_or_default(),
            comm: info.name,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(cmdline: &str) -> bool {
        ProcInfo {
            comm: "java".into(),
            cmdline: cmdline.into(),
            ..ProcInfo::default()
        }
        .is_game()
    }

    #[test]
    fn is_game() {
        let updates = "/home/u/.cristalix/updates/Minigames-java21";
        assert!(game(&format!(
            "/usr/bin/java -Xmx4G -cp {updates}/minecraft.jar:{updates}/libraries/guava.jar net.minecraft.client.main.Main --username Steve"
        )));
        assert!(game(
            "/usr/bin/java -cp /mnt/games/cristalix/Minigames/minecraft.jar ru.cristalix.client.Main"
        ));
        assert!(!game(
            "/usr/bin/java -jar /home/u/.local/share/AccountChanger/Cristalix.jar"
        ));
        assert!(!game(
            "/usr/bin/java -jar /home/u/.local/share/AccountChanger/CristalixLauncher.jar"
        ));
        assert!(!game("/usr/bin/java -jar /opt/jetbrains/idea.jar"));
        assert!(!game(""));
    }

    #[test]
    fn is_java() {
        let proc = |comm: &str, cmdline: &str| ProcInfo {
            comm: comm.into(),
            cmdline: cmdline.into(),
            ..ProcInfo::default()
        };
        assert!(proc("java", "").is_java());
        assert!(proc("", "/home/u/.cristalix/runtime/bin/java -jar x.jar").is_java());
        assert!(proc("java", "/very/long/path/to/jdk-21.0.1+12/bin/java -version").is_java());
        assert!(!proc("bash", "/bin/bash -c echo").is_java());
    }

    #[test]
    fn scan_procs_finds_self() {
        let procs = scan_procs(true);
        assert!(!procs.is_empty());
        let me = procs
            .iter()
            .find(|proc| proc.pid == std::process::id())
            .expect("scan_procs did not report the test process itself");
        assert!(!me.cmdline.is_empty());
        assert_ne!(me.ppid, 0);
    }
}
