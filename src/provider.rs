use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::Provider;

const PROVIDERS: [&str; 2] = ["github", "linear"];

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
pub(crate) struct Config {
    #[serde(default)]
    disabled_providers: Vec<String>,
}

impl Config {
    pub(crate) fn enabled(&self, name: &str) -> bool {
        !self.disabled_providers.iter().any(|p| p == name)
    }

    fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_providers.retain(|p| p != name);
        if !enabled {
            self.disabled_providers.push(name.to_owned());
        }
    }
}

pub fn run(cmd: Cmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Cmd::List => {
            let config = load();
            let statuses: serde_json::Map<String, serde_json::Value> = PROVIDERS
                .iter()
                .map(|name| (name.to_string(), json!({"enabled": config.enabled(name)})))
                .collect();
            println!("{}", serde_json::Value::Object(statuses));
            Ok(())
        }
        Cmd::Enable { provider } => set_enabled(provider, true),
        Cmd::Disable { provider } => set_enabled(provider, false),
    }
}

fn set_enabled(provider: Provider, enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let path = config_path(xdg.as_deref(), std::env::home_dir().as_deref())
        .ok_or("could not determine the config path")?;
    let mut config = read(&path).unwrap_or_default();
    config.set_enabled(provider.as_str(), enabled);
    write(&path, &config)?;
    println!(
        "{}",
        json!({"provider": provider.as_str(), "enabled": enabled})
    );
    Ok(())
}

pub(crate) fn ensure_enabled(
    config: &Config,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if config.enabled(name) {
        Ok(())
    } else {
        Err(format!("{name} is disabled; run `foac provider enable {name}` to enable it").into())
    }
}

pub(crate) fn load() -> Config {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    config_path(xdg.as_deref(), std::env::home_dir().as_deref())
        .and_then(|path| read(&path))
        .unwrap_or_default()
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
