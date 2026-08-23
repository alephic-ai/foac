use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use self_update::cargo_crate_version;
use serde::{Deserialize, Serialize};

const CHECK_TTL_SECS: u64 = 24 * 60 * 60;
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/lra/foac/releases/latest";

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty());
    // Ask /releases/latest for the tag to install: that endpoint never returns
    // draft or prerelease releases, unlike the /releases listing self_update
    // walks by default, which includes in-progress drafts when authenticated.
    let tag = latest_release_tag(token.as_deref(), None)?;
    if tag == format!("v{}", cargo_crate_version!()) {
        println!("Already up to date ({})", cargo_crate_version!());
        return Ok(());
    }
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner("lra")
        .repo_name("foac")
        .bin_name("foac")
        .show_download_progress(true)
        .no_confirm(true)
        .release_tag(&tag)
        // Assets pair an archive with a .sha256 file per target, and
        // self_update takes the first name containing the target — which is
        // the checksum, alphabetically. Require the archive extension.
        .asset_identifier(if cfg!(windows) { ".zip" } else { ".tar.gz" })
        .current_version(cargo_crate_version!());
    if let Some(token) = token {
        builder.auth_token(token);
    }
    let status = builder.build()?.update()?;
    if status.is_updated() {
        println!("Updated to {}", status.version());
    } else {
        println!("Already up to date ({})", status.version());
    }
    Ok(())
}

/// Best-effort notice after a command. Never fails the caller, never writes stdout.
pub fn notify_if_outdated() {
    let _ = notify_if_outdated_inner();
}

fn notify_if_outdated_inner() -> Result<(), Box<dyn std::error::Error>> {
    if !should_check(
        std::env::var_os("FOAC_NO_UPDATE_CHECK").is_some(),
        std::env::var_os("CI").is_some(),
    ) {
        return Ok(());
    }
    let xdg = std::env::var("XDG_CACHE_HOME").ok();
    let Some(path) = cache_path(xdg.as_deref(), std::env::home_dir().as_deref()) else {
        return Ok(());
    };
    let cache = read_cache(&path);
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let token = std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty());
    let (outcome, new_cache) = check(now, cargo_crate_version!(), cache.as_ref(), || {
        latest_release_tag(token.as_deref(), Some(Duration::from_secs(2)))
    });
    if let Some(new_cache) = new_cache {
        let _ = write_cache(&path, &new_cache);
    }
    if let CheckOutcome::Outdated { current, latest } = outcome {
        eprintln!(
            "A new release of foac is available: {current} → {latest}\nTo upgrade, run: foac update"
        );
    }
    Ok(())
}

fn latest_release_tag(
    token: Option<&str>,
    timeout: Option<Duration>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut builder = reqwest::blocking::Client::builder();
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    let mut request = builder
        .build()?
        .get(LATEST_RELEASE_URL)
        .header("User-Agent", "foac");
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let latest: serde_json::Value = request.send()?.error_for_status()?.json()?;
    latest["tag_name"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "no tag_name in the latest GitHub release".into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Cache {
    checked_at: u64,
    latest: String,
}

#[derive(Debug, PartialEq, Eq)]
enum CheckOutcome {
    Skip,
    UpToDate,
    Outdated { current: String, latest: String },
}

fn should_check(no_update_check: bool, ci: bool) -> bool {
    !no_update_check && !ci
}

fn cache_path(xdg_cache_home: Option<&str>, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(xdg) = xdg_cache_home.filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("foac/update-check.json"));
    }
    Some(home?.join(".cache/foac/update-check.json"))
}

fn read_cache(path: &Path) -> Option<Cache> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write_cache(path: &Path, cache: &Cache) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec(cache)?)?;
    Ok(())
}

fn check(
    now: u64,
    current: &str,
    cache: Option<&Cache>,
    fetch: impl FnOnce() -> Result<String, Box<dyn std::error::Error>>,
) -> (CheckOutcome, Option<Cache>) {
    let (latest, new_cache) = match cache {
        Some(c) if now.saturating_sub(c.checked_at) < CHECK_TTL_SECS => (c.latest.clone(), None),
        _ => match fetch() {
            Ok(tag) => {
                let new = Cache {
                    checked_at: now,
                    latest: tag.clone(),
                };
                (tag, Some(new))
            }
            Err(_) => return (CheckOutcome::Skip, None),
        },
    };
    if latest == format!("v{current}") {
        (CheckOutcome::UpToDate, new_cache)
    } else {
        let latest = latest.strip_prefix('v').unwrap_or(&latest).to_string();
        (
            CheckOutcome::Outdated {
                current: current.to_string(),
                latest,
            },
            new_cache,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fetch_ok(tag: &str) -> impl FnOnce() -> Result<String, Box<dyn std::error::Error>> {
        let tag = tag.to_string();
        move || Ok(tag)
    }

    fn fetch_err() -> impl FnOnce() -> Result<String, Box<dyn std::error::Error>> {
        || Err("network".into())
    }

    fn fetch_must_not_run() -> impl FnOnce() -> Result<String, Box<dyn std::error::Error>> {
        || panic!("must not fetch on a fresh cache")
    }

    #[test]
    fn cache_hit_same_version_is_up_to_date() {
        let cache = Cache {
            checked_at: 10,
            latest: "v0.6.1".into(),
        };
        let (outcome, new_cache) = check(11, "0.6.1", Some(&cache), fetch_must_not_run());
        assert_eq!(outcome, CheckOutcome::UpToDate);
        assert_eq!(new_cache, None);
    }

    #[test]
    fn cache_hit_newer_tag_is_outdated() {
        let cache = Cache {
            checked_at: 10,
            latest: "v0.7.0".into(),
        };
        let (outcome, new_cache) = check(11, "0.6.1", Some(&cache), fetch_must_not_run());
        assert_eq!(
            outcome,
            CheckOutcome::Outdated {
                current: "0.6.1".into(),
                latest: "0.7.0".into()
            }
        );
        assert_eq!(new_cache, None);
    }

    #[test]
    fn stale_cache_fetches_and_rewrites() {
        let now = 2_000_000_000;
        let cache = Cache {
            checked_at: now - CHECK_TTL_SECS - 1,
            latest: "v0.6.1".into(),
        };
        let (outcome, new_cache) = check(now, "0.6.1", Some(&cache), fetch_ok("v0.7.0"));
        assert_eq!(
            outcome,
            CheckOutcome::Outdated {
                current: "0.6.1".into(),
                latest: "0.7.0".into()
            }
        );
        assert_eq!(
            new_cache,
            Some(Cache {
                checked_at: now,
                latest: "v0.7.0".into()
            })
        );
    }

    #[test]
    fn missing_cache_fetch_same_version_is_up_to_date() {
        let now = 100;
        let (outcome, new_cache) = check(now, "0.6.1", None, fetch_ok("v0.6.1"));
        assert_eq!(outcome, CheckOutcome::UpToDate);
        assert_eq!(
            new_cache,
            Some(Cache {
                checked_at: now,
                latest: "v0.6.1".into()
            })
        );
    }

    #[test]
    fn fetch_error_skips_without_writing() {
        let (outcome, new_cache) = check(100, "0.6.1", None, fetch_err());
        assert_eq!(outcome, CheckOutcome::Skip);
        assert_eq!(new_cache, None);
    }

    #[test]
    fn should_check_respects_opt_out_and_ci() {
        assert!(should_check(false, false));
        assert!(!should_check(true, false));
        assert!(!should_check(false, true));
        assert!(!should_check(true, true));
    }

    #[test]
    fn cache_path_prefers_xdg() {
        assert_eq!(
            cache_path(Some("/tmp/xdg"), Some(Path::new("/home/u"))).unwrap(),
            PathBuf::from("/tmp/xdg/foac/update-check.json")
        );
    }

    #[test]
    fn cache_path_falls_back_to_home() {
        assert_eq!(
            cache_path(None, Some(Path::new("/home/u"))).unwrap(),
            PathBuf::from("/home/u/.cache/foac/update-check.json")
        );
    }

    #[test]
    fn cache_path_treats_empty_xdg_as_unset() {
        assert_eq!(
            cache_path(Some(""), Some(Path::new("/home/u"))).unwrap(),
            PathBuf::from("/home/u/.cache/foac/update-check.json")
        );
    }

    #[test]
    fn cache_path_none_without_home_or_xdg() {
        assert_eq!(cache_path(None, None), None);
    }

    #[test]
    fn cache_json_round_trip() {
        let cache = Cache {
            checked_at: 1_720_000_000,
            latest: "v0.7.0".into(),
        };
        let parsed: Cache = serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
        assert_eq!(parsed, cache);
        let parsed: Cache =
            serde_json::from_str(r#"{"checked_at":1720000000,"latest":"v0.7.0"}"#).unwrap();
        assert_eq!(parsed, cache);
    }

    #[test]
    fn read_cache_ignores_missing_and_corrupt_files() {
        let dir = std::env::temp_dir().join(format!("foac-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.json");
        let corrupt = dir.join("corrupt.json");
        std::fs::write(&corrupt, "not-json").unwrap();
        assert_eq!(read_cache(&missing), None);
        assert_eq!(read_cache(&corrupt), None);
        let written = Cache {
            checked_at: 1,
            latest: "v1.0.0".into(),
        };
        let path = dir.join("update-check.json");
        write_cache(&path, &written).unwrap();
        assert_eq!(read_cache(&path), Some(written));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
