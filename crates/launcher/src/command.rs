use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) fn detached(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    #[cfg(unix)]
    command.process_group(0);
    command
}

pub(crate) fn in_dir_of(command: &mut Command, file: &Path) {
    if let Some(dir) = file.parent() {
        command.current_dir(dir);
    }
}
