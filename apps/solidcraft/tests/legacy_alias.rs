//! `solidcraft` is Polykrom's earlier name: it runs `polykrom` with the same arguments and exit
//! status, so scripts and shortcuts that start it keep working (docs/compatibility.md).

use std::process::Command;

#[test]
fn solidcraft_forwards_to_polykrom() {
    let version = format!("solidcraft {}", env!("CARGO_PKG_VERSION"));
    for bin in [env!("CARGO_BIN_EXE_solidcraft"), env!("CARGO_BIN_EXE_polykrom")] {
        let out = Command::new(bin).arg("--version").output().unwrap();
        assert!(out.status.success(), "{bin}: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), version, "{bin}");
    }
}
