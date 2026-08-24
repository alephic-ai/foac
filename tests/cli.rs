//! End-to-end tests running the compiled binary. Only commands that parse
//! cleanly are safe here: any parse error makes `run()` probe provider auth
//! (keychain reads, a `gh` subprocess), which must not happen in tests.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_CONFIG_DIR: AtomicUsize = AtomicUsize::new(0);

fn foac(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_foac"));
    cmd.args(args)
        .env("FOAC_NO_UPDATE_CHECK", "1")
        // Point config reads at an empty dir so the user's real config
        // (disabled providers, credentials) can't affect the output.
        .env("XDG_CONFIG_HOME", env!("CARGO_TARGET_TMPDIR"));
    cmd
}

fn config_home(test_name: &str) -> PathBuf {
    let id = NEXT_CONFIG_DIR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("foac-cli-{test_name}-{}-{id}", std::process::id()))
}

fn malformed_settings(test_name: &str) -> (PathBuf, PathBuf, Vec<u8>) {
    let config_home = config_home(test_name);
    let settings_path = config_home.join("foac/config.toml");
    let original = b"disabled_providers = [\"github\"".to_vec();
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(&settings_path, &original).unwrap();
    (config_home, settings_path, original)
}

#[test]
fn version_prints_the_cargo_version() {
    let out = foac(&["version"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        concat!(env!("CARGO_PKG_VERSION"), "\n")
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn provider_list_defaults_to_all_enabled_json() {
    let out = foac(&["provider", "list"]).output().unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for name in ["github", "linear", "sentry", "slack"] {
        assert_eq!(json[name]["enabled"], serde_json::Value::Bool(true));
    }
}

#[test]
fn provider_list_reports_a_malformed_config() {
    let (config_home, config_path, _) = malformed_settings("provider-list");

    let out = foac(&["provider", "list"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains(&config_path.display().to_string()));
    assert!(stderr.contains("could not parse settings file"));

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn sentry_auth_status_represents_a_malformed_config_as_an_error() {
    let (config_home, config_path, _) = malformed_settings("sentry-status");

    let out = foac(&["auth", "sentry", "status"])
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("LINEAR_API_KEY")
        .env_remove("GITHUB_TOKEN")
        .env("SENTRY_AUTH_TOKEN", "environment-token")
        .env_remove("SENTRY_URL")
        .env_remove("SLACK_BOT_TOKEN")
        .env_remove("SLACK_USER_TOKEN")
        .output()
        .unwrap();

    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["sentry"]["status"], "error");
    assert_eq!(report["sentry"]["source"], "environment");
    let error = report["sentry"]["error"].as_str().unwrap();
    assert!(error.contains(&config_path.display().to_string()));
    assert!(error.contains("could not parse settings file"));

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn provider_mutation_does_not_overwrite_a_malformed_config() {
    let (config_home, config_path, original) = malformed_settings("provider-mutation");

    let out = foac(&["provider", "disable", "github"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert_eq!(std::fs::read(&config_path).unwrap(), original);

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn malformed_credentials_do_not_block_settings_mutations() {
    let config_home = config_home("credential-isolation");
    let credentials_path = config_home.join("foac/credentials.json");
    let original = br#"{"github":"sensitive-token","linear":42}"#;
    std::fs::create_dir_all(credentials_path.parent().unwrap()).unwrap();
    std::fs::write(&credentials_path, original).unwrap();

    let out = foac(&["provider", "disable", "github"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();

    assert!(out.status.success());
    assert_eq!(std::fs::read(&credentials_path).unwrap(), original);
    assert!(config_home.join("foac/config.toml").exists());

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn sentry_login_preflights_both_stores_before_mutating() {
    let (config_home, settings_path, settings_original) = malformed_settings("sentry-login");
    let credentials_path = config_home.join("foac/credentials.json");
    let credentials_original = br#"{"sentry":"old-token"}"#;
    std::fs::write(&credentials_path, credentials_original).unwrap();

    let out = foac(&["auth", "sentry", "login", "--host", "sentry.example.com"])
        .env("XDG_CONFIG_HOME", &config_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(b"new-token\n")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(!out.status.success());
    assert_eq!(std::fs::read(&settings_path).unwrap(), settings_original);
    assert_eq!(
        std::fs::read(&credentials_path).unwrap(),
        credentials_original
    );

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn lone_legacy_config_is_ignored_and_unchanged() {
    let config_home = config_home("legacy-config");
    let legacy_path = config_home.join("foac/config.json");
    let original = br#"{"disabled_providers":["github"],"credentials":{"linear":"legacy-token"}}"#;
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    std::fs::write(&legacy_path, original).unwrap();

    let providers = foac(&["provider", "list"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();
    assert!(providers.status.success());
    let providers: serde_json::Value = serde_json::from_slice(&providers.stdout).unwrap();
    assert_eq!(providers["github"]["enabled"], true);

    let auth = foac(&["auth", "linear", "status"])
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("LINEAR_API_KEY")
        .output()
        .unwrap();
    assert!(auth.status.success());
    assert!(auth.stderr.is_empty());
    let auth: serde_json::Value = serde_json::from_slice(&auth.stdout).unwrap();
    assert_eq!(auth["linear"]["status"], "unauthenticated");
    assert_eq!(std::fs::read(&legacy_path).unwrap(), original);

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn slack_login_suggests_an_app_manifest() {
    let out = foac(&["auth", "slack", "login"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("https://api.slack.com/apps"));
    let manifest = stderr
        .split_once("Suggested manifest:\n")
        .unwrap()
        .1
        .split_once("\nInstall the app")
        .unwrap()
        .0;
    let manifest: serde_json::Value = serde_json::from_str(manifest).unwrap();
    assert_eq!(
        manifest["oauth_config"]["scopes"]["bot"],
        serde_json::json!([
            "channels:history",
            "channels:read",
            "chat:write",
            "groups:history",
            "groups:read",
            "im:history",
            "im:read",
            "mpim:history",
            "mpim:read",
            "reactions:write",
            "users:read",
            "users:read.email"
        ])
    );
    assert_eq!(
        manifest["oauth_config"]["scopes"]["user"],
        serde_json::json!(["search:read"])
    );
}
