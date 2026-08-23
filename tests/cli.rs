//! End-to-end tests running the compiled binary. Only commands that parse
//! cleanly are safe here: any parse error makes `run()` probe provider auth
//! (keychain reads, a `gh` subprocess), which must not happen in tests.

use std::process::Command;

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
    for name in ["github", "linear", "sentry"] {
        assert_eq!(json[name]["enabled"], serde_json::Value::Bool(true));
    }
}
