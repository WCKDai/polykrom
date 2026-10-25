//! `solidcraft`: SolidCraft's name for the desktop app, kept so scripts and shortcuts that start it
//! keep working. It runs `polykrom` (next to this executable, else on PATH) with the same arguments
//! and standard streams and exits with its status (docs/compatibility.md).
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use std::process::{Command, ExitCode};

const TARGET: &str = "polykrom";

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
            eprintln!("solidcraft: could not run {}: {e}", program.to_string_lossy());
            ExitCode::FAILURE
        }
    }
}
