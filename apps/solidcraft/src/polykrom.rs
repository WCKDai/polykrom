//! `polykrom`: Polykrom's name for the desktop app. It runs `solidcraft` (next to this executable, else on PATH)
//! with the same arguments and standard streams and exits with its status, until the packages move
//! to the new name (docs/compatibility.md).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use std::process::{Command, ExitCode};

const TARGET: &str = "solidcraft";

fn main() -> ExitCode {
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(format!("{TARGET}{}", std::env::consts::EXE_SUFFIX))))
        .filter(|p| p.is_file());
    let program = sibling.map(|p| p.into_os_string()).unwrap_or_else(|| TARGET.into());
    match Command::new(&program).args(std::env::args_os().skip(1)).status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            None => ExitCode::FAILURE,
        },
        Err(e) => {
            eprintln!("polykrom: could not run {}: {e}", program.to_string_lossy());
            ExitCode::FAILURE
        }
    }
}
