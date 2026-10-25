//! The CLI's earlier names forward to `polykrom-cli`: `solidcraft-cli` silently (MCP registrations
//! use it), `solvecraft-cli` with a deprecation note.
use std::process::Command;

#[test]
fn legacy_alias_forwards_with_a_deprecation_note() {
    let out = Command::new(env!("CARGO_BIN_EXE_solvecraft-cli")).arg("--version").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), format!("polykrom-cli {}", env!("CARGO_PKG_VERSION")));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("deprecated") && stderr.contains("polykrom-cli"), "{stderr}");
}

#[test]
fn legacy_alias_passes_the_exit_status_through() {
    let real = Command::new(env!("CARGO_BIN_EXE_polykrom-cli")).arg("no-such-command").output().unwrap();
    for alias in [env!("CARGO_BIN_EXE_solvecraft-cli"), env!("CARGO_BIN_EXE_solidcraft-cli")] {
        let out = Command::new(alias).arg("no-such-command").output().unwrap();
        assert_eq!(out.status.code(), real.status.code(), "{alias}");
        assert_eq!(out.stdout, real.stdout, "{alias}");
    }
}

/// SolidCraft's name runs Polykrom's CLI and says nothing of it: an MCP client registered with
/// `solidcraft-cli mcp` would show a note on stderr as an error (#98).
#[test]
fn solidcraft_cli_forwards_without_a_note() {
    let out = Command::new(env!("CARGO_BIN_EXE_solidcraft-cli")).arg("--version").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), format!("polykrom-cli {}", env!("CARGO_PKG_VERSION")));
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
}
