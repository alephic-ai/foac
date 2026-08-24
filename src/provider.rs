use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::Provider;

pub const PROVIDERS: [&str; 4] = ["github", "linear", "sentry", "slack"];

#[derive(Subcommand)]
pub enum Cmd {
    /// List providers and whether they are enabled
    List,
    /// Enable a provider
    Enable { provider: Provider },
    /// Disable a provider: hide it and refuse its commands
    Disable { provider: Provider },
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    disabled_providers: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    credentials: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sentry_url: Option<String>,
}

impl Config {
    pub fn enabled(&self, name: &str) -> bool {
        !self.disabled_providers.iter().any(|p| p == name)
    }

    fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_providers.retain(|p| p != name);
        if !enabled {
            self.disabled_providers.push(name.to_owned());
        }
    }

    pub(crate) fn credential(&self, name: &str) -> Option<&str> {
        self.credentials.get(name).map(String::as_str)
    }

    pub(crate) fn set_credential(&mut self, name: &str, token: String) {
        self.credentials.insert(name.to_owned(), token);
    }

    pub(crate) fn remove_credential(&mut self, name: &str) -> bool {
        self.credentials.remove(name).is_some()
    }

    pub(crate) fn sentry_url(&self) -> Option<&str> {
        self.sentry_url.as_deref().filter(|url| !url.is_empty())
    }

    pub(crate) fn set_sentry_url(&mut self, url: Option<String>) {
        self.sentry_url = url;
    }
}

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Cmd::List => {
            crate::output::print(&statuses(&load()?), format);
            Ok(())
        }
        Cmd::Enable { provider } => set_enabled(provider, true, format),
        Cmd::Disable { provider } => set_enabled(provider, false, format),
    }
}

fn statuses(config: &Config) -> serde_json::Value {
    serde_json::Value::Object(
        PROVIDERS
            .iter()
            .map(|name| (name.to_string(), json!({"enabled": config.enabled(name)})))
            .collect(),
    )
}

fn set_enabled(
    provider: Provider,
    enabled: bool,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load()?;
    config.set_enabled(provider.as_str(), enabled);
    save(&config)?;
    crate::output::print_highlighting(&statuses(&config), format, provider.as_str());
    Ok(())
}

pub fn ensure_enabled(config: &Config, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if config.enabled(name) {
        Ok(())
    } else {
        Err(format!("{name} is disabled; run `foac provider enable {name}` to enable it").into())
    }
}

pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
    let path = path().ok_or("could not determine the config path")?;
    load_from(&path)
}

pub(crate) fn save(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let path = path().ok_or("could not determine the config path")?;
    write(&path, config)
}

fn path() -> Option<PathBuf> {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    config_path(xdg.as_deref(), std::env::home_dir().as_deref())
}

fn config_path(xdg_config_home: Option<&str>, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(xdg) = xdg_config_home.filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("foac/config.json"));
    }
    Some(home?.join(".config/foac/config.json"))
}

fn load_from(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    Ok(read(path)?.unwrap_or_default())
}

fn read(path: &Path) -> Result<Option<Config>, Box<dyn std::error::Error>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!("could not read config file {}: {error}", path.display()).into());
        }
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|error| {
        let cause = match error.classify() {
            serde_json::error::Category::Io => "JSON I/O error",
            serde_json::error::Category::Syntax => "invalid JSON syntax",
            serde_json::error::Category::Data => "invalid config data",
            serde_json::error::Category::Eof => "unexpected end of JSON input",
        };
        format!(
            "could not parse config file {}: {cause} at line {} column {}",
            path.display(),
            error.line(),
            error.column()
        )
        .into()
    })
}

fn write(path: &Path, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(config)?;
    write_private(path, |file| {
        file.set_len(0)?;
        file.write_all(&bytes)
    })?;
    Ok(())
}

fn write_private(
    path: &Path,
    write_body: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;

    // Existing files may have permissive modes, so tighten them before the
    // callback can truncate the file or write credentials.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    write_body(&mut file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_prefers_xdg() {
        assert_eq!(
            config_path(Some("/tmp/xdg"), Some(Path::new("/home/u"))).unwrap(),
            PathBuf::from("/tmp/xdg/foac/config.json")
        );
    }

    #[test]
    fn config_path_falls_back_to_home() {
        assert_eq!(
            config_path(None, Some(Path::new("/home/u"))).unwrap(),
            PathBuf::from("/home/u/.config/foac/config.json")
        );
    }

    #[test]
    fn config_path_treats_empty_xdg_as_unset() {
        assert_eq!(
            config_path(Some(""), Some(Path::new("/home/u"))).unwrap(),
            PathBuf::from("/home/u/.config/foac/config.json")
        );
    }

    #[test]
    fn config_path_none_without_home_or_xdg() {
        assert_eq!(config_path(None, None), None);
    }

    #[test]
    fn config_round_trip() {
        let dir = std::env::temp_dir().join(format!("foac-provider-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        assert_eq!(read(&path).unwrap(), None);
        assert!(Config::default().enabled("github"));

        let mut config = Config::default();
        config.set_enabled("github", false);
        config.set_enabled("github", false);
        write(&path, &config).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"disabled_providers\": [\n    \"github\"\n  ]\n}"
        );

        let config = read(&path).unwrap().unwrap();
        assert!(!config.enabled("github"));
        assert!(config.enabled("linear"));

        let mut config = config;
        config.set_enabled("github", true);
        assert!(config.enabled("github"));
        assert!(config.disabled_providers.is_empty());

        config.set_credential("linear", "lin_api_token".into());
        write(&path, &config).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let config = read(&path).unwrap().unwrap();
        assert_eq!(config.credential("linear"), Some("lin_api_token"));
        assert_eq!(config.credential("github"), None);

        let mut config = config;
        config.set_credential("slack_bot", "xoxb-bot".into());
        config.set_credential("slack_user", "xoxp-user".into());
        write(&path, &config).unwrap();
        let config = read(&path).unwrap().unwrap();
        assert_eq!(config.credential("slack_bot"), Some("xoxb-bot"));
        assert_eq!(config.credential("slack_user"), Some("xoxp-user"));

        let mut config = config;
        assert!(config.remove_credential("slack_bot"));
        assert!(config.remove_credential("slack_user"));
        assert!(config.remove_credential("linear"));
        assert!(!config.remove_credential("linear"));
        write(&path, &config).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"disabled_providers\": []\n}"
        );

        config.set_sentry_url(Some("https://sentry.example.com".into()));
        write(&path, &config).unwrap();
        let config = read(&path).unwrap().unwrap();
        assert_eq!(config.sentry_url(), Some("https://sentry.example.com"));
        assert_eq!(Config::default().sentry_url(), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn config_write_pretty_prints_and_round_trips_populated_fields() {
        let dir =
            std::env::temp_dir().join(format!("foac-provider-pretty-test-{}", std::process::id()));
        let path = dir.join("config.json");

        let mut config = Config::default();
        config.set_enabled("github", false);
        config.set_credential("slack_user", "xoxp-user".into());
        config.set_credential("linear", "lin_api_token".into());
        config.set_sentry_url(Some("https://sentry.example.com".into()));

        write(&path, &config).unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            serde_json::to_string_pretty(&config).unwrap().as_bytes()
        );
        assert_eq!(read(&path).unwrap(), Some(config));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn private_writer_creates_file_with_private_mode_before_writing() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "foac-provider-private-create-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        write_private(&path, |file| {
            let mode = file.metadata().unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
            assert_eq!(std::fs::read(&path).unwrap(), b"");
            file.write_all(b"secret")
        })
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"secret");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn private_writer_tightens_existing_file_before_replacing_contents() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "foac-provider-private-existing-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, b"old secret").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();

        write_private(&path, |file| {
            let mode = file.metadata().unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
            assert_eq!(std::fs::read(&path).unwrap(), b"old secret");
            file.set_len(0)?;
            file.write_all(b"new secret")
        })
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new secret");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn config_read_distinguishes_missing_unreadable_and_malformed_files() {
        let dir =
            std::env::temp_dir().join(format!("foac-provider-read-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.json");
        assert_eq!(load_from(&missing).unwrap(), Config::default());

        let unreadable = dir.join("unreadable.json");
        std::fs::create_dir(&unreadable).unwrap();
        let error = load_from(&unreadable).unwrap_err().to_string();
        assert!(error.contains("could not read config file"));
        assert!(error.contains(&unreadable.display().to_string()));

        let malformed = dir.join("malformed.json");
        std::fs::write(
            &malformed,
            br#"{"credentials":"sensitive-token-not-for-errors"}"#,
        )
        .unwrap();
        let error = load_from(&malformed).unwrap_err().to_string();
        assert!(error.contains("could not parse config file"));
        assert!(error.contains(&malformed.display().to_string()));
        assert!(error.contains("invalid config data at line 1 column"));
        assert!(!error.contains("sensitive-token-not-for-errors"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ensure_enabled_reports_disabled_providers() {
        let mut config = Config::default();
        assert!(ensure_enabled(&config, "github").is_ok());
        config.set_enabled("github", false);
        let error = ensure_enabled(&config, "github").unwrap_err().to_string();
        assert_eq!(
            error,
            "github is disabled; run `foac provider enable github` to enable it"
        );
    }

    #[test]
    fn statuses_lists_every_provider() {
        let mut config = Config::default();
        config.set_enabled("sentry", false);
        assert_eq!(
            statuses(&config),
            json!({
                "github": {"enabled": true},
                "linear": {"enabled": true},
                "sentry": {"enabled": false},
                "slack": {"enabled": true},
            })
        );
    }
}
