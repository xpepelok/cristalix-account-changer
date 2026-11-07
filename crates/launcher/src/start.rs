use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::command::{detached, in_dir_of};
use crate::error::{LauncherError, Result};

fn ensure_exists(path: &Path) -> Result<()> {
    std::fs::metadata(path)
        .map(drop)
        .map_err(LauncherError::NotFound)
}

fn launcher_command(exe: &Path) -> Result<Command> {
    ensure_exists(exe)?;
    let mut command = detached(exe);
    in_dir_of(&mut command, exe);
    Ok(command)
}

pub fn start_launcher(exe: &Path) -> Result<()> {
    launcher_command(exe)?.spawn()?;
    Ok(())
}

pub fn spawn_launcher(exe: &Path, home: Option<&Path>) -> Result<Child> {
    let mut command = launcher_command(exe)?;
    if let Some(home) = home {
        command.env("USERPROFILE", home).env("APPDATA", home);
    }
    Ok(command.spawn()?)
}

fn jar_command(java: &Path, jar: &Path, home: Option<&Path>) -> Result<Command> {
    ensure_exists(jar)?;
    let mut command = detached(java);
    if let Some(home) = home {
        let mut property = std::ffi::OsString::from("-Duser.home=");
        property.push(home);
        command.arg(property).env("APPDATA", home);
    }
    command.arg("-jar").arg(jar);
    in_dir_of(&mut command, jar);
    Ok(command)
}

pub fn start_launcher_jar(java: &Path, jar: &Path) -> Result<()> {
    jar_command(java, jar, None)?.spawn()?;
    Ok(())
}

pub(crate) fn spawn_launcher_jar(
    java: &Path,
    jar: &Path,
    home: Option<&Path>,
) -> Result<(Child, mpsc::Receiver<()>)> {
    let mut command = jar_command(java, jar, home)?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let (broken, signal) = mpsc::channel(1);
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(scan_jar_broken(stdout, broken.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(scan_jar_broken(stderr, broken));
    }
    Ok((child, signal))
}

async fn scan_jar_broken(stream: impl AsyncRead + Unpin, broken: mpsc::Sender<()>) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if is_broken_jar_line(&line) {
            let _ = broken.try_send(());
        }
    }
}

fn is_broken_jar_line(line: &str) -> bool {
    let low = line.to_lowercase();
    low.contains("invalid or corrupt jarfile") || low.contains("unable to access jarfile")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_jar_lines_are_detected() {
        assert!(is_broken_jar_line(
            "Error: Invalid or corrupt jarfile C:\\x\\Cristalix.jar"
        ));
        assert!(is_broken_jar_line(
            "Error: Unable to access jarfile Cristalix.jar"
        ));
        assert!(!is_broken_jar_line("[main] INFO launcher started"));
    }

    #[test]
    fn missing_launcher_reports_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let err = launcher_command(&dir.path().join("missing.exe")).unwrap_err();
        assert!(err.to_string().starts_with("launcher not found: "));
    }
}
