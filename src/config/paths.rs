//! Config file location.
//!
//! `hop` resolves its config to a single path on every supported platform
//! (macOS and Linux — resume uses Unix `exec(2)`, so there is no Windows case):
//!
//! 1. `$HOP_CONFIG`, used verbatim as a path to a config *file*, when set.
//! 2. otherwise `<xdg-config-home>/hop/config.toml`, where `<xdg-config-home>`
//!    is `$XDG_CONFIG_HOME` when set to an absolute path, else `~/.config`.
//!
//! `$XDG_CONFIG_HOME` and `~/.config` are mutually exclusive rather than
//! searched in order: the XDG base directory spec has `$XDG_CONFIG_HOME`
//! *replace* `~/.config`, not outrank it.
//!
//! Deliberately not `directories::ProjectDirs`: its `config_dir()` is
//! platform-idiomatic, so on macOS it resolves to
//! `~/Library/Application Support/dev.hop.hop`, while the README has always
//! documented `~/.config/hop/config.toml`. That mismatch meant a macOS config
//! was read from a path no user was told about, and one written at the
//! documented path was silently ignored.
//!
//! `resolve` is pure: no environment reads, no filesystem access (not even
//! existence checks — that is the caller's separate concern), and no
//! `cfg!(target_os = ...)`. `ConfigPathInputs::from_env` is the only place that
//! touches the environment, which keeps precedence unit-testable without
//! mutating process state.

use std::path::{Path, PathBuf};

/// Everything `resolve` needs, so resolution stays a pure function.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfigPathInputs {
    /// `$HOP_CONFIG` — an explicit config *file*, not a directory.
    pub hop_config: Option<PathBuf>,
    /// `$XDG_CONFIG_HOME` — honored only when absolute, per the XDG spec.
    pub xdg_config_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

/// Reads an environment variable, treating empty (and non-UTF-8) as unset.
fn env_path(key: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(key)?;
    if raw.is_empty() { None } else { Some(PathBuf::from(raw)) }
}

impl ConfigPathInputs {
    /// The only environment- and platform-aware entry point in this module.
    pub fn from_env() -> ConfigPathInputs {
        ConfigPathInputs {
            hop_config: env_path("HOP_CONFIG"),
            xdg_config_home: env_path("XDG_CONFIG_HOME"),
            home: directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()),
        }
    }
}

/// The config file path, or `None` when there is no override and no home dir.
pub fn resolve(inputs: &ConfigPathInputs) -> Option<PathBuf> {
    if let Some(explicit) = &inputs.hop_config {
        return Some(explicit.clone());
    }
    let config_home = match &inputs.xdg_config_home {
        // A relative `$XDG_CONFIG_HOME` must be ignored (XDG spec), rather than
        // joined onto the working directory.
        Some(dir) if dir.is_absolute() => dir.clone(),
        Some(_) | None => inputs.home.as_ref()?.join(".config"),
    };
    Some(config_home.join("hop").join("config.toml"))
}

/// Path a pre-0.4.0 macOS `hop config init` would have written, when it still
/// exists. Only `config init` consults this, to warn that the file is no longer
/// read; nothing in the load path knows the location exists.
pub fn legacy_macos_config(home: &Path) -> Option<PathBuf> {
    let path =
        home.join("Library").join("Application Support").join("dev.hop.hop").join("config.toml");
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inputs with a fixed home and no environment overrides.
    fn with_home(home: &Path) -> ConfigPathInputs {
        ConfigPathInputs { home: Some(home.to_path_buf()), ..ConfigPathInputs::default() }
    }

    #[test]
    fn resolves_to_xdg_path_under_home() {
        let home = tempfile::tempdir().unwrap();
        let got = resolve(&with_home(home.path())).unwrap();
        assert_eq!(got, home.path().join(".config/hop/config.toml"));
    }

    /// The bug this module exists to fix: on macOS the resolved path must be the
    /// documented XDG one, never `~/Library/Application Support`.
    #[test]
    fn never_resolves_into_application_support() {
        let home = tempfile::tempdir().unwrap();
        let inputs = with_home(home.path());
        let got = resolve(&inputs).unwrap();
        assert!(
            !got.to_string_lossy().contains("Application Support"),
            "resolved into the legacy macOS location: {}",
            got.display()
        );
    }

    #[test]
    fn hop_config_wins_over_xdg_config_home() {
        let home = tempfile::tempdir().unwrap();
        let inputs = ConfigPathInputs {
            hop_config: Some(PathBuf::from("/somewhere/custom.toml")),
            xdg_config_home: Some(PathBuf::from("/xdg")),
            home: Some(home.path().to_path_buf()),
        };
        assert_eq!(resolve(&inputs).unwrap(), PathBuf::from("/somewhere/custom.toml"));
    }

    #[test]
    fn hop_config_is_returned_even_when_missing() {
        let home = tempfile::tempdir().unwrap();
        let missing = home.path().join("nope/absent.toml");
        let inputs =
            ConfigPathInputs { hop_config: Some(missing.clone()), ..with_home(home.path()) };
        assert!(!missing.exists());
        assert_eq!(resolve(&inputs).unwrap(), missing);
    }

    /// `$XDG_CONFIG_HOME` *replaces* `~/.config`; it does not merely outrank it.
    /// An existing file under `~/.config` must not pull resolution back there.
    #[test]
    fn xdg_config_home_replaces_home_config_even_when_home_file_exists() {
        let home = tempfile::tempdir().unwrap();
        let home_config = home.path().join(".config/hop");
        std::fs::create_dir_all(&home_config).unwrap();
        std::fs::write(home_config.join("config.toml"), "search_mode = \"raw\"").unwrap();

        let inputs = ConfigPathInputs {
            xdg_config_home: Some(PathBuf::from("/xdg")),
            ..with_home(home.path())
        };
        assert_eq!(resolve(&inputs).unwrap(), PathBuf::from("/xdg/hop/config.toml"));
    }

    #[test]
    fn relative_xdg_config_home_is_ignored() {
        let home = tempfile::tempdir().unwrap();
        let inputs = ConfigPathInputs {
            xdg_config_home: Some(PathBuf::from("relative/config")),
            ..with_home(home.path())
        };
        assert_eq!(resolve(&inputs).unwrap(), home.path().join(".config/hop/config.toml"));
    }

    #[test]
    fn empty_env_values_are_treated_as_unset() {
        let home = tempfile::tempdir().unwrap();
        // `from_env` maps empty vars to `None`; assert the shape it produces
        // resolves to the home-relative path rather than to `""`.
        let inputs = with_home(home.path());
        assert!(inputs.hop_config.is_none());
        assert!(inputs.xdg_config_home.is_none());
        assert_eq!(resolve(&inputs).unwrap(), home.path().join(".config/hop/config.toml"));
    }

    #[test]
    fn empty_xdg_config_home_falls_back_to_home() {
        let home = tempfile::tempdir().unwrap();
        // Defensive: even if an empty value reaches `resolve` (it is relative,
        // not absolute), it must not become a bare `hop/config.toml`.
        let inputs =
            ConfigPathInputs { xdg_config_home: Some(PathBuf::new()), ..with_home(home.path()) };
        assert_eq!(resolve(&inputs).unwrap(), home.path().join(".config/hop/config.toml"));
    }

    #[test]
    fn no_home_and_no_override_resolves_to_none() {
        let inputs = ConfigPathInputs::default();
        assert_eq!(resolve(&inputs), None);
    }

    #[test]
    fn no_home_still_honors_absolute_xdg_config_home() {
        let inputs = ConfigPathInputs {
            xdg_config_home: Some(PathBuf::from("/xdg")),
            ..ConfigPathInputs::default()
        };
        assert_eq!(resolve(&inputs).unwrap(), PathBuf::from("/xdg/hop/config.toml"));
    }

    #[test]
    fn legacy_macos_config_found_only_when_file_exists() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(legacy_macos_config(home.path()), None);

        let legacy = home.path().join("Library/Application Support/dev.hop.hop");
        std::fs::create_dir_all(&legacy).unwrap();
        let file = legacy.join("config.toml");
        std::fs::write(&file, "search_mode = \"raw\"").unwrap();
        assert_eq!(legacy_macos_config(home.path()), Some(file));
    }

    #[test]
    fn legacy_macos_config_ignores_a_directory_at_that_path() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(
            home.path().join("Library/Application Support/dev.hop.hop/config.toml"),
        )
        .unwrap();
        assert_eq!(legacy_macos_config(home.path()), None);
    }
}
