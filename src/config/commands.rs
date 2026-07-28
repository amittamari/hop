use anyhow::{Context, Result};
use std::path::Path;

use super::{Config, ConfigPathInputs, config_path, config_template, paths};

fn resolve_path() -> Result<std::path::PathBuf> {
    config_path().context("could not determine config directory")
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    Ok(())
}

fn write_template(path: &Path) -> Result<()> {
    ensure_parent(path)?;
    std::fs::write(path, config_template()).with_context(|| format!("writing {}", path.display()))
}

pub fn cmd_path() -> Result<()> {
    let path = resolve_path()?;
    println!("{}", path.display());
    Ok(())
}

/// Notice shown when scaffolding a config while a pre-0.4.0 macOS config is
/// still sitting in `~/Library/Application Support`. Kept as a pure formatter so
/// it is testable on every platform, not just macOS.
fn stale_legacy_notice(legacy: &Path, current: &Path) -> String {
    format!(
        "note: found an old config at {}\n      \
         hop no longer reads that location. To keep those settings:\n      \
         mv {legacy:?} {current:?}",
        legacy.display()
    )
}

/// The stale-config notice for this machine, if one applies. The macOS gate
/// lives here rather than in `paths` so the path helper stays platform-neutral.
fn legacy_notice_for_env(current: &Path) -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let home = ConfigPathInputs::from_env().home?;
    let legacy = paths::legacy_macos_config(&home)?;
    Some(stale_legacy_notice(&legacy, current))
}

pub fn cmd_init() -> Result<()> {
    let path = resolve_path()?;
    if path.exists() {
        eprintln!("config already exists: {}", path.display());
        return Ok(());
    }
    if let Some(notice) = legacy_notice_for_env(&path) {
        eprintln!("{notice}");
    }
    write_template(&path)?;
    eprintln!("created {}", path.display());
    Ok(())
}

pub fn cmd_edit() -> Result<()> {
    let path = resolve_path()?;
    if !path.exists() {
        write_template(&path)?;
        eprintln!("created {}", path.display());
    }
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_owned());
    let parts =
        shlex::split(&editor).ok_or_else(|| anyhow::anyhow!("unterminated quote in $EDITOR"))?;
    let (cmd, args) = parts.split_first().ok_or_else(|| anyhow::anyhow!("$EDITOR is empty"))?;
    let status = std::process::Command::new(cmd)
        .args(args)
        .arg(&path)
        .status()
        .with_context(|| format!("launching editor: {editor}"))?;
    if !status.success() {
        anyhow::bail!("editor exited with {status}");
    }
    Ok(())
}

pub fn cmd_show() -> Result<()> {
    let cfg = Config::load()?;
    let text = toml::to_string_pretty(&cfg).context("serializing config")?;
    print!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn cmd_init_creates_template() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert!(!path.exists());
        write_template(&path).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[display]"));
    }

    #[test]
    fn cmd_show_outputs_valid_toml() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let reparsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(reparsed.display.icons, cfg.display.icons);
        assert_eq!(reparsed.display.width_pct, cfg.display.width_pct);
        assert_eq!(reparsed.search_mode, cfg.search_mode);
    }

    #[test]
    fn cmd_show_with_overrides() {
        let toml_in = r#"
            [display]
            icons = false
            width_pct = 60
        "#;
        let cfg = Config::from_toml_str(toml_in).unwrap();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let reparsed: Config = toml::from_str(&text).unwrap();
        assert!(!reparsed.display.icons);
        assert_eq!(reparsed.display.width_pct, 60);
    }

    #[test]
    fn stale_legacy_notice_names_both_paths() {
        let legacy = Path::new("/Users/x/Library/Application Support/dev.hop.hop/config.toml");
        let current = Path::new("/Users/x/.config/hop/config.toml");
        let notice = stale_legacy_notice(legacy, current);
        assert!(notice.contains("Application Support/dev.hop.hop/config.toml"), "{notice}");
        assert!(notice.contains("/Users/x/.config/hop/config.toml"), "{notice}");
        assert!(notice.contains("no longer reads"), "{notice}");
        // The move hint must quote the path — `Application Support` has a space.
        assert!(notice.contains("mv \"/Users/x/Library/Application Support"), "{notice}");
    }

    /// The notice is advisory: scaffolding still happens and the old file is
    /// left untouched.
    #[test]
    fn init_still_scaffolds_alongside_a_legacy_config() {
        let home = tempfile::tempdir().unwrap();
        let legacy_dir = home.path().join("Library/Application Support/dev.hop.hop");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let legacy = legacy_dir.join("config.toml");
        std::fs::write(&legacy, "search_mode = \"raw\"").unwrap();

        let current = home.path().join(".config/hop/config.toml");
        assert!(paths::legacy_macos_config(home.path()).is_some());
        assert!(!stale_legacy_notice(&legacy, &current).is_empty());

        write_template(&current).unwrap();
        assert!(current.exists());
        // The legacy file is never read, moved, or rewritten.
        assert_eq!(std::fs::read_to_string(&legacy).unwrap(), "search_mode = \"raw\"");
    }

    #[test]
    fn no_legacy_notice_without_a_legacy_file() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(paths::legacy_macos_config(home.path()), None);
    }

    #[test]
    fn write_template_is_idempotent_guard() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_template(&path).unwrap();
        let mut f = std::fs::OpenOptions::new().write(true).truncate(true).open(&path).unwrap();
        f.write_all(b"custom content").unwrap();
        drop(f);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "custom content");
    }
}
