//! End-to-end tests running the compiled binary. Only commands that parse
//! cleanly are safe here: any parse error makes `run()` probe provider auth
//! (keychain reads, a `gh` subprocess), which must not happen in tests.

use std::process::{Command, Stdio};

fn foac(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_foac"));
    cmd.args(args)
        .env("FOAC_NO_UPDATE_CHECK", "1")
        // Point config reads at an empty dir so the user's real config
        // (disabled providers, credentials) can't affect the output.
        .env("XDG_CONFIG_HOME", env!("CARGO_TARGET_TMPDIR"));
    cmd
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
