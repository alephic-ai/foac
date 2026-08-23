use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::Provider;

const PROVIDERS: [&str; 3] = ["github", "linear", "sentry"];

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
            let config = load();
            let statuses: serde_json::Map<String, serde_json::Value> = PROVIDERS
                .iter()
                .map(|name| (name.to_string(), json!({"enabled": config.enabled(name)})))
                .collect();
            crate::output::print(&serde_json::Value::Object(statuses), format);
            Ok(())
        }
        Cmd::Enable { provider } => set_enabled(provider, true, format),
        Cmd::Disable { provider } => set_enabled(provider, false, format),
    }
}

fn set_enabled(
    provider: Provider,
    enabled: bool,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load();
    config.set_enabled(provider.as_str(), enabled);
    save(&config)?;
    crate::output::print(
        &json!({"provider": provider.as_str(), "enabled": enabled}),
        format,
    );
    Ok(())
}

pub fn ensure_enabled(config: &Config, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if config.enabled(name) {
        Ok(())
    } else {
        Err(format!("{name} is disabled; run `foac provider enable {name}` to enable it").into())
    }
}

pub fn load() -> Config {
    path().and_then(|path| read(&path)).unwrap_or_default()
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

fn read(path: &Path) -> Option<Config> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write(path: &Path, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec(config)?)?;
    // The file can hold credentials, so keep it readable by the owner only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
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
        let corrupt = dir.join("corrupt.json");
        std::fs::write(&corrupt, "not-json").unwrap();

        assert_eq!(read(&path), None);
        assert_eq!(read(&corrupt), None);
        assert!(Config::default().enabled("github"));

        let mut config = Config::default();
        config.set_enabled("github", false);
        config.set_enabled("github", false);
        write(&path, &config).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"disabled_providers":["github"]}"#
        );

        let config = read(&path).unwrap();
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
        let config = read(&path).unwrap();
        assert_eq!(config.credential("linear"), Some("lin_api_token"));
        assert_eq!(config.credential("github"), None);

        let mut config = config;
        assert!(config.remove_credential("linear"));
        assert!(!config.remove_credential("linear"));
        write(&path, &config).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"disabled_providers":[]}"#
        );

        config.set_sentry_url(Some("https://sentry.example.com".into()));
        write(&path, &config).unwrap();
        let config = read(&path).unwrap();
        assert_eq!(config.sentry_url(), Some("https://sentry.example.com"));
        assert_eq!(Config::default().sentry_url(), None);

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
}
