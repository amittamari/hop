use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/amittamari/hop/releases/latest";
const CACHE_TTL_SECS: u64 = 86_400; // 24 hours

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    Homebrew,
    CargoInstall,
    Unknown,
}

pub struct UpdateAvailable {
    pub current: semver::Version,
    pub latest: semver::Version,
    pub install_method: InstallMethod,
}

#[derive(Serialize, Deserialize)]
struct UpdateCache {
    latest_version: String,
    checked_at: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn detect_install_method_from_path(path: &Path) -> InstallMethod {
    let s = path.to_string_lossy();
    if s.contains("/Cellar/") || s.contains("/homebrew/") {
        InstallMethod::Homebrew
    } else if s.contains("/.cargo/bin/") {
        InstallMethod::CargoInstall
    } else {
        InstallMethod::Unknown
    }
}

fn detect_install_method() -> InstallMethod {
    std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::canonicalize(p).ok())
        .map(|p| detect_install_method_from_path(&p))
        .unwrap_or(InstallMethod::Unknown)
}

fn read_cache(path: &Path) -> Option<UpdateCache> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_cache(path: &Path, cache: &UpdateCache) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, data);
    }
}

fn fetch_latest_version() -> Option<String> {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(5)))
        .timeout_recv_body(Some(std::time::Duration::from_secs(5)))
        .user_agent(concat!("hop/", env!("CARGO_PKG_VERSION")))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let body: serde_json::Value = agent
        .get(GITHUB_RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    let tag = body.get("tag_name")?.as_str()?;
    Some(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

pub fn check_for_update(cache_path: &Path) -> Option<UpdateAvailable> {
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).ok()?;
    let install_method = detect_install_method();

    let latest_str = if let Some(cache) = read_cache(cache_path) {
        if now_secs().saturating_sub(cache.checked_at) < CACHE_TTL_SECS {
            cache.latest_version
        } else {
            let v = fetch_latest_version()?;
            write_cache(
                cache_path,
                &UpdateCache {
                    latest_version: v.clone(),
                    checked_at: now_secs(),
                },
            );
            v
        }
    } else {
        let v = fetch_latest_version()?;
        write_cache(
            cache_path,
            &UpdateCache {
                latest_version: v.clone(),
                checked_at: now_secs(),
            },
        );
        v
    };

    let latest = semver::Version::parse(&latest_str).ok()?;
    if latest > current {
        Some(UpdateAvailable {
            current,
            latest,
            install_method,
        })
    } else {
        None
    }
}

pub fn upgrade_message(info: &UpdateAvailable) -> String {
    let instruction = match info.install_method {
        InstallMethod::Homebrew => "  brew upgrade hop".to_string(),
        _ => "  See https://github.com/amittamari/hop/releases/latest".to_string(),
    };
    format!(
        "hop: update available v{} \u{2192} v{}\n{}",
        info.current, info.latest, instruction
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detect_homebrew_cellar() {
        let path = PathBuf::from("/opt/homebrew/Cellar/hop/0.2.3/bin/hop");
        assert_eq!(
            detect_install_method_from_path(&path),
            InstallMethod::Homebrew
        );
    }

    #[test]
    fn detect_homebrew_prefix() {
        let path = PathBuf::from("/opt/homebrew/bin/hop");
        assert_eq!(
            detect_install_method_from_path(&path),
            InstallMethod::Homebrew
        );
    }

    #[test]
    fn detect_homebrew_linux_prefix() {
        let path = PathBuf::from("/home/linuxbrew/.linuxbrew/Cellar/hop/0.2.3/bin/hop");
        assert_eq!(
            detect_install_method_from_path(&path),
            InstallMethod::Homebrew
        );
    }

    #[test]
    fn detect_cargo_install() {
        let path = PathBuf::from("/Users/me/.cargo/bin/hop");
        assert_eq!(
            detect_install_method_from_path(&path),
            InstallMethod::CargoInstall
        );
    }

    #[test]
    fn detect_unknown() {
        let path = PathBuf::from("/usr/local/bin/hop");
        assert_eq!(
            detect_install_method_from_path(&path),
            InstallMethod::Unknown
        );
    }

    #[test]
    fn upgrade_message_homebrew() {
        let info = UpdateAvailable {
            current: semver::Version::new(0, 2, 3),
            latest: semver::Version::new(0, 3, 0),
            install_method: InstallMethod::Homebrew,
        };
        let msg = upgrade_message(&info);
        assert!(msg.contains("v0.2.3"));
        assert!(msg.contains("v0.3.0"));
        assert!(msg.contains("brew upgrade hop"));
    }

    #[test]
    fn upgrade_message_cargo() {
        let info = UpdateAvailable {
            current: semver::Version::new(0, 2, 3),
            latest: semver::Version::new(0, 2, 4),
            install_method: InstallMethod::CargoInstall,
        };
        let msg = upgrade_message(&info);
        assert!(msg.contains("releases/latest"));
    }

    #[test]
    fn upgrade_message_unknown() {
        let info = UpdateAvailable {
            current: semver::Version::new(0, 2, 3),
            latest: semver::Version::new(1, 0, 0),
            install_method: InstallMethod::Unknown,
        };
        let msg = upgrade_message(&info);
        assert!(msg.contains("releases/latest"));
    }

    #[test]
    fn cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update_check.json");

        assert!(read_cache(&path).is_none());

        let cache = UpdateCache {
            latest_version: "0.3.0".to_string(),
            checked_at: now_secs(),
        };
        write_cache(&path, &cache);

        let loaded = read_cache(&path).unwrap();
        assert_eq!(loaded.latest_version, "0.3.0");
    }

    #[test]
    fn no_update_when_current_is_latest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update_check.json");

        let cache = UpdateCache {
            latest_version: env!("CARGO_PKG_VERSION").to_string(),
            checked_at: now_secs(),
        };
        write_cache(&path, &cache);

        assert!(check_for_update(&path).is_none());
    }

    #[test]
    fn no_update_when_current_is_newer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update_check.json");

        let cache = UpdateCache {
            latest_version: "0.0.1".to_string(),
            checked_at: now_secs(),
        };
        write_cache(&path, &cache);

        assert!(check_for_update(&path).is_none());
    }

    #[test]
    fn update_when_latest_is_newer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update_check.json");

        let cache = UpdateCache {
            latest_version: "99.0.0".to_string(),
            checked_at: now_secs(),
        };
        write_cache(&path, &cache);

        let info = check_for_update(&path).unwrap();
        assert_eq!(info.latest, semver::Version::new(99, 0, 0));
    }
}
