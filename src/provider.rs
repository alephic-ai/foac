use std::collections::BTreeMap;
use std::fs::File;
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

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Settings {
    disabled_providers: Vec<String>,
    sentry_url: Option<String>,
}

impl Settings {
    pub fn enabled(&self, name: &str) -> bool {
        !self.disabled_providers.iter().any(|p| p == name)
    }

    fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_providers.retain(|p| p != name);
        if !enabled {
            self.disabled_providers.push(name.to_owned());
        }
    }

    pub(crate) fn sentry_url(&self) -> Option<&str> {
        self.sentry_url.as_deref().filter(|url| !url.is_empty())
    }

    pub(crate) fn set_sentry_url(&mut self, url: Option<String>) {
        self.sentry_url = url;
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Credential {
    Linear,
    Github,
    Sentry,
    SlackBot,
    SlackUser,
}

impl Credential {
    fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Github => "github",
            Self::Sentry => "sentry",
            Self::SlackBot => "slack_bot",
            Self::SlackUser => "slack_user",
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Credentials {
    values: BTreeMap<String, String>,
}

impl Credentials {
    pub(crate) fn get(&self, credential: Credential) -> Option<&str> {
        self.values.get(credential.as_str()).map(String::as_str)
    }

    fn set(&mut self, credential: Credential, token: String) {
        self.values.insert(credential.as_str().to_owned(), token);
    }

    fn remove(&mut self, credential: Credential) -> bool {
        self.values.remove(credential.as_str()).is_some()
    }
}

/// Settings and credentials still serialize through this compatibility model
/// until their persistence formats are separated.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    disabled_providers: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    credentials: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sentry_url: Option<String>,
}

impl ConfigFile {
    fn settings(&self) -> Settings {
        Settings {
            disabled_providers: self.disabled_providers.clone(),
            sentry_url: self.sentry_url.clone(),
        }
    }

    fn set_settings(&mut self, settings: Settings) {
        self.disabled_providers = settings.disabled_providers;
        self.sentry_url = settings.sentry_url;
    }

    fn credentials(&self) -> Credentials {
        Credentials {
            values: self.credentials.clone(),
        }
    }

    fn set_credentials(&mut self, credentials: Credentials) {
        self.credentials = credentials.values;
    }
}

pub struct SettingsStore;

impl SettingsStore {
    pub fn load(&self) -> Result<Settings, Box<dyn std::error::Error>> {
        let path = path().ok_or("could not determine the config path")?;
        load_settings_from(&path)
    }

    fn set_enabled(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<Settings, Box<dyn std::error::Error>> {
        let path = path().ok_or("could not determine the config path")?;
        set_enabled_at(&path, name, enabled)
    }

    pub(crate) fn set_sentry_url(
        &self,
        url: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = path().ok_or("could not determine the config path")?;
        set_sentry_url_at(&path, url)
    }
}

pub(crate) struct CredentialStore;

impl CredentialStore {
    pub(crate) fn load(&self) -> Result<Credentials, Box<dyn std::error::Error>> {
        let path = path().ok_or("could not determine the config path")?;
        load_credentials_from(&path)
    }

    pub(crate) fn set_many(
        &self,
        credentials: &[(Credential, &str)],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = path().ok_or("could not determine the config path")?;
        set_credentials_at(&path, credentials)
    }

    pub(crate) fn delete_many(
        &self,
        credentials: &[Credential],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let path = path().ok_or("could not determine the config path")?;
        delete_credentials_at(&path, credentials)
    }
}

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Cmd::List => {
            crate::output::print(&statuses(&SettingsStore.load()?), format);
            Ok(())
        }
        Cmd::Enable { provider } => set_enabled(provider, true, format),
        Cmd::Disable { provider } => set_enabled(provider, false, format),
    }
}

fn statuses(settings: &Settings) -> serde_json::Value {
    serde_json::Value::Object(
        PROVIDERS
            .iter()
            .map(|name| (name.to_string(), json!({"enabled": settings.enabled(name)})))
            .collect(),
    )
}

fn set_enabled(
    provider: Provider,
    enabled: bool,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = SettingsStore.set_enabled(provider.as_str(), enabled)?;
    crate::output::print_highlighting(&statuses(&settings), format, provider.as_str());
    Ok(())
}

pub fn ensure_enabled(settings: &Settings, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if settings.enabled(name) {
        Ok(())
    } else {
        Err(format!("{name} is disabled; run `foac provider enable {name}` to enable it").into())
    }
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

fn load_from(path: &Path) -> Result<ConfigFile, Box<dyn std::error::Error>> {
    Ok(read(path)?.unwrap_or_default())
}

fn read(path: &Path) -> Result<Option<ConfigFile>, Box<dyn std::error::Error>> {
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

fn write(path: &Path, config: &ConfigFile) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(config)?;
    write_private(path, |file| file.write_all(&bytes))?;
    Ok(())
}

fn load_settings_from(path: &Path) -> Result<Settings, Box<dyn std::error::Error>> {
    Ok(load_from(path)?.settings())
}

fn set_enabled_at(
    path: &Path,
    name: &str,
    enabled: bool,
) -> Result<Settings, Box<dyn std::error::Error>> {
    let mut config = load_from(path)?;
    let mut settings = config.settings();
    settings.set_enabled(name, enabled);
    config.set_settings(settings);
    write(path, &config)?;
    Ok(config.settings())
}

fn set_sentry_url_at(path: &Path, url: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_from(path)?;
    let mut settings = config.settings();
    settings.set_sentry_url(url);
    config.set_settings(settings);
    write(path, &config)
}

fn load_credentials_from(path: &Path) -> Result<Credentials, Box<dyn std::error::Error>> {
    Ok(load_from(path)?.credentials())
}

fn set_credentials_at(
    path: &Path,
    updates: &[(Credential, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_from(path)?;
    let mut credentials = config.credentials();
    for (credential, token) in updates {
        credentials.set(*credential, (*token).to_owned());
    }
    config.set_credentials(credentials);
    write(path, &config)
}

fn delete_credentials_at(
    path: &Path,
    removals: &[Credential],
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut config = load_from(path)?;
    let mut credentials = config.credentials();
    let mut removed = false;
    for credential in removals {
        removed = credentials.remove(*credential) || removed;
    }
    if removed {
        config.set_credentials(credentials);
        write(path, &config)?;
    }
    Ok(removed)
}

fn write_private(
    path: &Path,
    write_body: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::Builder::new()
        .prefix(".foac-config-")
        .tempfile_in(dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    write_body(temp.as_file_mut())?;

    // into_temp_path closes the handle before persist atomically replaces the
    // destination. On Windows, tempfile implements this with MoveFileExW and
    // MOVEFILE_REPLACE_EXISTING rather than std::fs::rename.
    let temp_path = temp.into_temp_path();
    temp_path.persist(path).map_err(|error| error.error)
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
    fn settings_updates_preserve_credentials_and_compatibility_bytes() {
        let dir =
            std::env::temp_dir().join(format!("foac-settings-store-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(
            &path,
            concat!(
                "{\n",
                "  \"disabled_providers\": [],\n",
                "  \"credentials\": {\n",
                "    \"github\": \"github-token\",\n",
                "    \"linear\": \"linear-token\",\n",
                "    \"sentry\": \"sentry-token\",\n",
                "    \"slack_bot\": \"xoxb-bot\",\n",
                "    \"slack_user\": \"xoxp-user\"\n",
                "  },\n",
                "  \"sentry_url\": \"https://sentry.example.com\"\n",
                "}"
            ),
        )
        .unwrap();

        set_enabled_at(&path, "github", false).unwrap();
        set_sentry_url_at(&path, Some("https://sentry.changed.example.com".into())).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "{\n",
                "  \"disabled_providers\": [\n",
                "    \"github\"\n",
                "  ],\n",
                "  \"credentials\": {\n",
                "    \"github\": \"github-token\",\n",
                "    \"linear\": \"linear-token\",\n",
                "    \"sentry\": \"sentry-token\",\n",
                "    \"slack_bot\": \"xoxb-bot\",\n",
                "    \"slack_user\": \"xoxp-user\"\n",
                "  },\n",
                "  \"sentry_url\": \"https://sentry.changed.example.com\"\n",
                "}"
            )
        );
        let credentials = load_credentials_from(&path).unwrap();
        assert_eq!(credentials.get(Credential::Github), Some("github-token"));
        assert_eq!(credentials.get(Credential::Linear), Some("linear-token"));
        assert_eq!(credentials.get(Credential::Sentry), Some("sentry-token"));
        assert_eq!(credentials.get(Credential::SlackBot), Some("xoxb-bot"));
        assert_eq!(credentials.get(Credential::SlackUser), Some("xoxp-user"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn credential_updates_preserve_settings_and_slack_tokens_are_independent() {
        let dir =
            std::env::temp_dir().join(format!("foac-credential-store-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(
            &path,
            concat!(
                "{\n",
                "  \"disabled_providers\": [\n",
                "    \"github\"\n",
                "  ],\n",
                "  \"sentry_url\": \"https://sentry.example.com\"\n",
                "}"
            ),
        )
        .unwrap();

        set_credentials_at(
            &path,
            &[
                (Credential::SlackBot, "xoxb-bot"),
                (Credential::SlackUser, "xoxp-user"),
            ],
        )
        .unwrap();
        assert!(delete_credentials_at(&path, &[Credential::SlackBot]).unwrap());

        let config = load_from(&path).unwrap();
        assert!(!config.settings().enabled("github"));
        assert_eq!(
            config.settings().sentry_url(),
            Some("https://sentry.example.com")
        );
        assert_eq!(
            config.credentials().get(Credential::SlackUser),
            Some("xoxp-user")
        );
        assert_eq!(config.credentials().get(Credential::SlackBot), None);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "{\n",
                "  \"disabled_providers\": [\n",
                "    \"github\"\n",
                "  ],\n",
                "  \"credentials\": {\n",
                "    \"slack_user\": \"xoxp-user\"\n",
                "  },\n",
                "  \"sentry_url\": \"https://sentry.example.com\"\n",
                "}"
            )
        );

        assert!(delete_credentials_at(&path, &[Credential::SlackUser]).unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "{\n",
                "  \"disabled_providers\": [\n",
                "    \"github\"\n",
                "  ],\n",
                "  \"sentry_url\": \"https://sentry.example.com\"\n",
                "}"
            )
        );

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
            assert!(!path.exists());
            file.write_all(b"secret")
        })
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"secret");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn private_writer_atomically_replaces_existing_file() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "foac-provider-private-existing-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, b"old secret").unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();

        write_private(&path, |file| {
            #[cfg(unix)]
            {
                let mode = file.metadata().unwrap().permissions().mode();
                assert_eq!(mode & 0o777, 0o600);
            }
            assert_eq!(std::fs::read(&path).unwrap(), b"old secret");
            file.write_all(b"new secret")
        })
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new secret");
        #[cfg(unix)]
        {
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn private_writer_keeps_existing_file_on_pre_replace_failure() {
        let dir = std::env::temp_dir().join(format!(
            "foac-provider-private-failure-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, b"old secret").unwrap();

        let error = write_private(&path, |file| {
            file.write_all(b"partial new secret")?;
            Err(std::io::Error::other("injected pre-replace failure"))
        })
        .unwrap_err();

        assert_eq!(error.to_string(), "injected pre-replace failure");
        assert_eq!(std::fs::read(&path).unwrap(), b"old secret");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn config_read_distinguishes_missing_unreadable_and_malformed_files() {
        let dir =
            std::env::temp_dir().join(format!("foac-provider-read-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.json");
        assert_eq!(load_from(&missing).unwrap(), ConfigFile::default());

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
        let mut settings = Settings::default();
        assert!(ensure_enabled(&settings, "github").is_ok());
        settings.set_enabled("github", false);
        let error = ensure_enabled(&settings, "github").unwrap_err().to_string();
        assert_eq!(
            error,
            "github is disabled; run `foac provider enable github` to enable it"
        );
    }

    #[test]
    fn statuses_lists_every_provider() {
        let mut settings = Settings::default();
        settings.set_enabled("sentry", false);
        assert_eq!(
            statuses(&settings),
            json!({
                "github": {"enabled": true},
                "linear": {"enabled": true},
                "sentry": {"enabled": false},
                "slack": {"enabled": true},
            })
        );
    }
}
