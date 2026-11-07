//! The installed identity: the names the packages, the window system and the per-user folders
//! know Polykrom by. Packaging reads the same values (packaging/env.sh, the platform files under
//! assets/platforms); `cargo xtask identity` checks that they agree.

/// The reverse-DNS app id: the macOS bundle identifier, the Linux desktop entry, AppStream and
/// Flatpak id, the icon name and the window's Wayland `app_id` / X11 class.
pub const APP_ID: &str = "app.polykrom.Polykrom";

/// The per-user data folder's name where the platform spells app folders in title case
/// (`%LOCALAPPDATA%\Polykrom`, `~/Library/Application Support/Polykrom`), and eframe's storage
/// folder.
pub const DATA_DIR_TITLE: &str = "Polykrom";

/// The per-user data folder's name on systems that spell app folders in lower case
/// (`$XDG_DATA_HOME/polykrom`).
pub const DATA_DIR_LOWER: &str = "polykrom";

#[cfg(test)]
mod tests {
    use super::*;

    /// No installed name says what Polykrom was forked from.
    #[test]
    fn the_identity_is_polykroms_own() {
        for name in [APP_ID, DATA_DIR_TITLE, DATA_DIR_LOWER] {
            let lower = name.to_ascii_lowercase();
            assert!(!lower.contains("solidcraft") && !lower.contains("solvecraft"), "{name}");
            assert!(lower.contains("polykrom"), "{name}");
        }
        assert_eq!(APP_ID, "app.polykrom.Polykrom");
    }
}
