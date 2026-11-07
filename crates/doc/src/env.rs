//! Polykrom's environment variables, read in one place: every variable is `POLYKROM_<NAME>`.
//! Nothing else is read (no fallback to the names of the fork's origin).

use std::ffi::OsString;

/// The prefix of every variable.
pub const PREFIX: &str = "POLYKROM_";

/// The variable's full name: `POLYKROM_<name>`.
pub fn name(name: &str) -> String {
    format!("{PREFIX}{name}")
}

/// `POLYKROM_<name>`. `name` is the part after the prefix, e.g. `"RECOVERY_DIR"`.
pub fn var_os(name: &str) -> Option<OsString> {
    std::env::var_os(self::name(name))
}

/// [`var_os`] as UTF-8; a value that is not valid UTF-8 counts as unset.
pub fn var(name: &str) -> Option<String> {
    var_os(name).and_then(|v| v.into_string().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_carry_the_polykrom_prefix_only() {
        assert_eq!(name("CONTROL_PORT"), "POLYKROM_CONTROL_PORT");
        assert!(!PREFIX.to_ascii_lowercase().contains("solidcraft") && !PREFIX.to_ascii_lowercase().contains("solvecraft"));
    }
}
