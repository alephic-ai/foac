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
        // (disabled providers, credentials) can't affect the output, and run
        // outside the checkout so a developer's `.foac.toml` can't either.
        .env("XDG_CONFIG_HOME", env!("CARGO_TARGET_TMPDIR"))
        .current_dir(std::env::temp_dir());
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
fn about_prints_the_banner_version_and_repository() {
    let out = foac(&["about"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("many services · many agents · one door"));
    assert!(stdout.contains(concat!("v", env!("CARGO_PKG_VERSION"))));
    assert!(stdout.contains("https://github.com/alephic-ai/foac"));
    // Piped stdout is not a TTY, so no ANSI escapes.
    assert!(!stdout.contains('\x1b'));
    assert!(out.stderr.is_empty());
}

#[test]
fn provider_list_defaults_to_all_enabled_json() {
    let out = foac(&["provider", "list"]).output().unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for name in [
        "confluence",
        "firecrawl",
        "github",
        "jira",
        "linear",
        "neon",
        "sentry",
        "slack",
        "vercel",
    ] {
        assert_eq!(json[name]["enabled"], serde_json::Value::Bool(true));
    }
}

#[test]
fn local_settings_toggle_providers_per_folder() {
    let root = config_home("local-settings");
    let sub = root.join("project/sub");
    std::fs::create_dir_all(&sub).unwrap();
    let local_path = root.join("project/.foac.toml");
    std::fs::write(&local_path, "disabled_providers = [\"github\"]\n").unwrap();

    // The file is found from a subfolder and overrides the (empty) global config.
    let out = foac(&["provider", "list"])
        .current_dir(&sub)
        .output()
        .unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["github"]["enabled"], false);
    assert_eq!(json["linear"]["enabled"], true);

    // A locally disabled provider refuses to run and names the file.
    let out = foac(&["github", "issue", "list", "--repo", "owner/repo"])
        .current_dir(&sub)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains(&local_path.display().to_string()));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_local_toggles_write_the_nearest_local_file() {
    let root = config_home("local-toggle");
    let xdg = root.join("xdg");
    std::fs::create_dir_all(xdg.join("foac")).unwrap();
    let global = xdg.join("foac/config.toml");
    std::fs::write(&global, "disabled_providers = [\"github\"]\n").unwrap();
    let work = root.join("project");
    let sub = work.join("sub");
    std::fs::create_dir_all(&sub).unwrap();

    // No local file yet: --local creates one in the working directory and the
    // local enable overrides the global disable.
    let out = foac(&["provider", "enable", "github", "--local"])
        .env("XDG_CONFIG_HOME", &xdg)
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["github"]["enabled"], true);
    assert_eq!(
        std::fs::read_to_string(work.join(".foac.toml")).unwrap(),
        "enabled_providers = [\"github\"]\n"
    );

    // From a subfolder, --local edits the nearest existing file, not cwd.
    let out = foac(&["provider", "disable", "github", "--local"])
        .env("XDG_CONFIG_HOME", &xdg)
        .current_dir(&sub)
        .output()
        .unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["github"]["enabled"], false);
    assert!(!sub.join(".foac.toml").exists());
    assert_eq!(
        std::fs::read_to_string(work.join(".foac.toml")).unwrap(),
        "enabled_providers = []\ndisabled_providers = [\"github\"]\n"
    );

    // The global config is never touched by --local.
    assert_eq!(
        std::fs::read_to_string(&global).unwrap(),
        "disabled_providers = [\"github\"]\n"
    );

    std::fs::remove_dir_all(root).unwrap();
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
fn sentry_auth_status_represents_malformed_credentials_as_an_error() {
    let config_home = config_home("sentry-status");
    let credentials_path = config_home.join("foac/credentials.json");
    std::fs::create_dir_all(credentials_path.parent().unwrap()).unwrap();
    std::fs::write(&credentials_path, br#"{"sentry":42}"#).unwrap();

    let out = foac(&["auth", "sentry", "status"])
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("SENTRY_AUTH_TOKEN")
        .env_remove("SENTRY_URL")
        .output()
        .unwrap();

    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["sentry"]["status"], "error");
    let error = report["sentry"]["error"].as_str().unwrap();
    assert!(error.contains(&credentials_path.display().to_string()));
    assert!(error.contains("could not parse credentials file"));

    std::fs::remove_dir_all(config_home).unwrap();
}

/// A slack command with no stored credentials fails at credential
/// resolution, and the error names the resolved instance, exposing the
/// flag > [defaults] precedence without any network I/O.
fn slack_instance_error(dir: &std::path::Path, args: &[&str]) -> String {
    let mut cmd = foac(args);
    cmd.env("XDG_CONFIG_HOME", config_home("slack-instance-empty"))
        .env_remove("SLACK_BOT_TOKEN")
        .env_remove("SLACK_USER_TOKEN")
        .current_dir(dir);
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    String::from_utf8(out.stderr).unwrap()
}

#[test]
fn instance_resolution_prefers_the_flag_over_folder_defaults() {
    let root = config_home("instance-resolution");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(".foac.toml"), "[defaults]\nslack = \"workd\"\n").unwrap();

    let base = &["slack", "conversation", "list"][..];
    let flagged = &["slack", "conversation", "list", "--instance", "workb"][..];

    // Folder default, then the flag over it.
    assert!(slack_instance_error(&root, base).contains("instance \"workd\""));
    assert!(slack_instance_error(&root, flagged).contains("instance \"workb\""));
    // The error names the login command for the instance.
    assert!(
        slack_instance_error(&root, flagged).contains("foac auth slack login --instance workb")
    );
    // The default instance keeps the environment-variable wording.
    let sub = root.join("no-defaults");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join(".foac.toml"), "").unwrap();
    assert!(slack_instance_error(&sub, base).contains("SLACK_BOT_TOKEN"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn disabled_instances_fail_closed_naming_the_local_file() {
    let root = config_home("instance-disable");
    std::fs::create_dir_all(&root).unwrap();
    let local_path = root.join(".foac.toml");
    std::fs::write(&local_path, "disabled_providers = [\"slack@workb\"]\n").unwrap();

    let stderr = slack_instance_error(
        &root,
        &["slack", "conversation", "list", "--instance", "workb"],
    );
    assert!(stderr.contains("slack@workb is disabled by"));
    assert!(stderr.contains(&local_path.display().to_string()));
    assert!(stderr.contains("foac provider enable slack --instance workb --local"));

    // The default instance is untouched by the instance toggle.
    let stderr = slack_instance_error(&root, &["slack", "conversation", "list"]);
    assert!(stderr.contains("SLACK_BOT_TOKEN"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_toggles_write_qualified_instance_names() {
    let root = config_home("instance-toggle");
    let xdg = root.join("xdg");
    std::fs::create_dir_all(xdg.join("foac")).unwrap();
    let work = root.join("project");
    std::fs::create_dir_all(&work).unwrap();

    let out = foac(&[
        "provider",
        "disable",
        "slack",
        "--instance",
        "workb",
        "--local",
    ])
    .env("XDG_CONFIG_HOME", &xdg)
    .current_dir(&work)
    .output()
    .unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    // The whole provider stays enabled; only the instance is toggled.
    assert_eq!(json["slack"]["enabled"], true);
    assert_eq!(
        std::fs::read_to_string(work.join(".foac.toml")).unwrap(),
        "disabled_providers = [\"slack@workb\"]\n"
    );

    let out = foac(&["provider", "disable", "slack", "--instance", "workb"])
        .env("XDG_CONFIG_HOME", &xdg)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        std::fs::read_to_string(xdg.join("foac/config.toml")).unwrap(),
        "disabled_providers = [\"slack@workb\"]\n"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn auth_logout_removes_only_the_named_instance() {
    let config_home = config_home("instance-logout");
    let credentials_path = config_home.join("foac/credentials.json");
    std::fs::create_dir_all(credentials_path.parent().unwrap()).unwrap();
    std::fs::write(
        &credentials_path,
        br#"{"slack":{"default":{"bot_token":"xoxb-a"},"workb":{"bot_token":"xoxb-b"}}}"#,
    )
    .unwrap();

    // A stored named instance appears in the provider list.
    let out = foac(&["provider", "list"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["slack@workb"]["enabled"], true);
    assert_eq!(json["slack@workb"]["authenticated"], true);

    let out = foac(&["auth", "slack", "logout", "--instance", "workb"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();
    assert!(out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["slack@workb"]["removed"], true);
    assert_eq!(
        std::fs::read_to_string(&credentials_path).unwrap(),
        concat!(
            "{\n",
            "  \"slack\": {\n",
            "    \"default\": {\n",
            "      \"bot_token\": \"xoxb-a\"\n",
            "    }\n",
            "  }\n",
            "}"
        )
    );

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
fn sentry_login_preflights_the_credential_store_before_mutating() {
    let config_home = config_home("sentry-login");
    let credentials_path = config_home.join("foac/credentials.json");
    let credentials_original = br#"{"sentry":42}"#;
    std::fs::create_dir_all(credentials_path.parent().unwrap()).unwrap();
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
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("could not read Sentry credential"));
    assert_eq!(
        std::fs::read(&credentials_path).unwrap(),
        credentials_original
    );

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn stray_config_json_is_ignored_and_unchanged() {
    let config_home = config_home("stray-config");
    let stray_path = config_home.join("foac/config.json");
    let original = br#"{"disabled_providers":["github"],"credentials":{"linear":"stray-token"}}"#;
    std::fs::create_dir_all(stray_path.parent().unwrap()).unwrap();
    std::fs::write(&stray_path, original).unwrap();

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
    assert_eq!(std::fs::read(&stray_path).unwrap(), original);

    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn jira_commands_name_the_first_missing_credential() {
    let out = foac(&["jira", "issue", "list"])
        .env_remove("ATLASSIAN_HOST")
        .env_remove("ATLASSIAN_EMAIL")
        .env_remove("ATLASSIAN_API_TOKEN")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("ATLASSIAN_HOST"));
    assert!(stderr.contains("foac auth jira login"));
}

#[test]
fn confluence_commands_name_their_own_login_command() {
    let out = foac(&["confluence", "page", "list"])
        .env_remove("ATLASSIAN_HOST")
        .env_remove("ATLASSIAN_EMAIL")
        .env_remove("ATLASSIAN_API_TOKEN")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("ATLASSIAN_HOST"));
    assert!(stderr.contains("foac auth confluence login"));
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
