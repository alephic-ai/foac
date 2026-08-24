use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde_json::json;
use toml_edit::{Array, DocumentMut, Item, Value, value};

use crate::auth::Provider;

pub const PROVIDERS: [&str; 4] = ["github", "linear", "sentry", "slack"];

/// Per-folder settings file, looked up from the working directory to `/`;
/// the nearest file wins and overrides the global toggles.
pub const LOCAL_SETTINGS_FILE: &str = ".foac.toml";

#[derive(Subcommand)]
pub enum Cmd {
    /// List providers and whether they are enabled
    List,
    /// Enable a provider
    Enable {
        provider: Provider,
        /// Toggle in the nearest .foac.toml instead of the global config
        /// (created in the working directory if none exists)
        #[arg(long)]
        local: bool,
    },
    /// Disable a provider: hide it and refuse its commands
    Disable {
        provider: Provider,
        /// Toggle in the nearest .foac.toml instead of the global config
        /// (created in the working directory if none exists)
        #[arg(long)]
        local: bool,
    },
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Settings {
    disabled_providers: Vec<String>,
    sentry_url: Option<String>,
    local: Option<LocalSettings>,
}

/// Toggles from the nearest per-folder settings file; a provider listed in
/// both arrays is enabled.
#[derive(Debug, PartialEq, Eq)]
struct LocalSettings {
    path: PathBuf,
    enabled_providers: Vec<String>,
    disabled_providers: Vec<String>,
}

impl Settings {
    pub fn enabled(&self, name: &str) -> bool {
        match &self.local {
            Some(local) if local.enabled_providers.iter().any(|p| p == name) => true,
            Some(local) if local.disabled_providers.iter().any(|p| p == name) => false,
            _ => !self.disabled_providers.iter().any(|p| p == name),
        }
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

pub struct SettingsStore;

impl SettingsStore {
    pub fn load(&self) -> Result<Settings, Box<dyn std::error::Error>> {
        let path = settings_path().ok_or("could not determine the settings path")?;
        let mut settings = load_settings_from(&path)?;
        settings.local = local_settings_for_cwd()?;
        Ok(settings)
    }

    fn set_enabled(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<Settings, Box<dyn std::error::Error>> {
        let path = settings_path().ok_or("could not determine the settings path")?;
        let mut settings = set_enabled_at(&path, name, enabled)?;
        // The printed map shows the effective state, local overrides included.
        settings.local = local_settings_for_cwd()?;
        Ok(settings)
    }

    /// Toggle in the nearest `.foac.toml` (the file in effect), creating one
    /// in the working directory if none exists up the tree.
    fn set_enabled_local(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<Settings, Box<dyn std::error::Error>> {
        let cwd = std::env::current_dir()
            .map_err(|error| format!("could not determine the working directory: {error}"))?;
        set_enabled_local_at(&nearest_local_settings_path(&cwd), name, enabled)?;
        self.load()
    }

    pub(crate) fn set_sentry_url(
        &self,
        url: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = settings_path().ok_or("could not determine the settings path")?;
        set_sentry_url_at(&path, url)
    }
}

pub(crate) struct CredentialStore;

impl CredentialStore {
    pub(crate) fn load(&self) -> Result<Credentials, Box<dyn std::error::Error>> {
        let path = credentials_path().ok_or("could not determine the credentials path")?;
        load_credentials_from(&path)
    }

    pub(crate) fn set_many(
        &self,
        credentials: &[(Credential, &str)],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = credentials_path().ok_or("could not determine the credentials path")?;
        set_credentials_at(&path, credentials)
    }

    pub(crate) fn delete_many(
        &self,
        credentials: &[Credential],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let path = credentials_path().ok_or("could not determine the credentials path")?;
        delete_credentials_at(&path, credentials)
    }
}

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Cmd::List => {
            crate::output::print(&statuses(&SettingsStore.load()?), format);
            Ok(())
        }
        Cmd::Enable { provider, local } => set_enabled(provider, true, local, format),
        Cmd::Disable { provider, local } => set_enabled(provider, false, local, format),
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
    local: bool,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = if local {
        SettingsStore.set_enabled_local(provider.as_str(), enabled)?
    } else {
        SettingsStore.set_enabled(provider.as_str(), enabled)?
    };
    crate::output::print_highlighting(&statuses(&settings), format, provider.as_str());
    Ok(())
}

pub fn ensure_enabled(settings: &Settings, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if settings.enabled(name) {
        return Ok(());
    }
    match &settings.local {
        Some(local) if local.disabled_providers.iter().any(|p| p == name) => Err(format!(
            "{name} is disabled by {}; run `foac provider enable {name} --local` to enable it",
            local.path.display()
        )
        .into()),
        _ => Err(
            format!("{name} is disabled; run `foac provider enable {name}` to enable it").into(),
        ),
    }
}

fn settings_path() -> Option<PathBuf> {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    store_path(
        xdg.as_deref(),
        std::env::home_dir().as_deref(),
        "config.toml",
    )
}

fn credentials_path() -> Option<PathBuf> {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    store_path(
        xdg.as_deref(),
        std::env::home_dir().as_deref(),
        "credentials.json",
    )
}

fn store_path(
    xdg_config_home: Option<&str>,
    home: Option<&Path>,
    filename: &str,
) -> Option<PathBuf> {
    if let Some(xdg) = xdg_config_home.filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("foac").join(filename));
    }
    Some(home?.join(".config/foac").join(filename))
}

fn read_file(path: &Path, kind: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("could not read {kind} file {}: {error}", path.display()).into()),
    }
}

fn load_settings_from(path: &Path) -> Result<Settings, Box<dyn std::error::Error>> {
    let Some(document) = read_settings_document(path)? else {
        return Ok(Settings::default());
    };
    settings_from_document(path, &document)
}

fn local_settings_for_cwd() -> Result<Option<LocalSettings>, Box<dyn std::error::Error>> {
    match std::env::current_dir() {
        Ok(dir) => load_local_settings(&dir),
        Err(_) => Ok(None),
    }
}

fn load_local_settings(start: &Path) -> Result<Option<LocalSettings>, Box<dyn std::error::Error>> {
    for dir in start.ancestors() {
        let path = dir.join(LOCAL_SETTINGS_FILE);
        let Some(document) = read_settings_document(&path)? else {
            continue;
        };
        return Ok(Some(LocalSettings {
            enabled_providers: string_array(&path, &document, "enabled_providers")?,
            disabled_providers: string_array(&path, &document, "disabled_providers")?,
            path,
        }));
    }
    Ok(None)
}

fn read_settings_document(path: &Path) -> Result<Option<DocumentMut>, Box<dyn std::error::Error>> {
    let Some(bytes) = read_file(path, "settings")? else {
        return Ok(None);
    };
    let text = String::from_utf8(bytes).map_err(|error| {
        format!(
            "could not parse settings file {}: invalid UTF-8 at byte {}",
            path.display(),
            error.utf8_error().valid_up_to()
        )
    })?;
    let document = text
        .parse::<DocumentMut>()
        .map_err(|error| settings_parse_error(path, &text, &error))?;
    Ok(Some(document))
}

fn set_enabled_at(
    path: &Path,
    name: &str,
    enabled: bool,
) -> Result<Settings, Box<dyn std::error::Error>> {
    let mut document = load_settings_document(path)?;
    let mut settings = settings_from_document(path, &document)?;
    settings.set_enabled(name, enabled);
    retain_recognized_settings(&mut document);
    set_provider_listed(&mut document, "disabled_providers", name, !enabled);
    write_settings(path, &document)?;
    Ok(settings)
}

fn nearest_local_settings_path(start: &Path) -> PathBuf {
    start
        .ancestors()
        .map(|dir| dir.join(LOCAL_SETTINGS_FILE))
        .find(|path| path.exists())
        .unwrap_or_else(|| start.join(LOCAL_SETTINGS_FILE))
}

fn set_enabled_local_at(
    path: &Path,
    name: &str,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut document = load_settings_document(path)?;
    string_array(path, &document, "enabled_providers")?;
    string_array(path, &document, "disabled_providers")?;
    document
        .as_table_mut()
        .retain(|key, _| matches!(key, "enabled_providers" | "disabled_providers"));
    set_provider_listed(&mut document, "enabled_providers", name, enabled);
    set_provider_listed(&mut document, "disabled_providers", name, !enabled);
    write_settings(path, &document)
}

fn set_sentry_url_at(path: &Path, url: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut document = load_settings_document(path)?;
    settings_from_document(path, &document)?;
    retain_recognized_settings(&mut document);
    match url {
        Some(url) => set_string(&mut document, "sentry_url", url),
        None => {
            document.remove("sentry_url");
        }
    }
    write_settings(path, &document)
}

fn load_credentials_from(path: &Path) -> Result<Credentials, Box<dyn std::error::Error>> {
    let Some(bytes) = read_file(path, "credentials")? else {
        return Ok(Credentials::default());
    };
    let values = serde_json::from_slice(&bytes).map_err(|error| {
        let cause = match error.classify() {
            serde_json::error::Category::Io => "JSON I/O error",
            serde_json::error::Category::Syntax => "invalid JSON syntax",
            serde_json::error::Category::Data => "invalid credential data",
            serde_json::error::Category::Eof => "unexpected end of JSON input",
        };
        format!(
            "could not parse credentials file {}: {cause} at line {} column {}",
            path.display(),
            error.line(),
            error.column()
        )
    })?;
    Ok(Credentials { values })
}

fn set_credentials_at(
    path: &Path,
    updates: &[(Credential, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut credentials = load_credentials_from(path)?;
    for (credential, token) in updates {
        credentials.set(*credential, (*token).to_owned());
    }
    write_credentials(path, &credentials)
}

fn delete_credentials_at(
    path: &Path,
    removals: &[Credential],
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut credentials = load_credentials_from(path)?;
    let mut removed = false;
    for credential in removals {
        removed = credentials.remove(*credential) || removed;
    }
    if removed {
        write_credentials(path, &credentials)?;
    }
    Ok(removed)
}

fn load_settings_document(path: &Path) -> Result<DocumentMut, Box<dyn std::error::Error>> {
    Ok(read_settings_document(path)?.unwrap_or_default())
}

fn settings_parse_error(
    path: &Path,
    text: &str,
    error: &toml_edit::TomlError,
) -> Box<dyn std::error::Error> {
    let position = error.span().map(|span| {
        let mut offset = span.start.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let prefix = &text[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix.rsplit_once('\n').map_or(prefix, |(_, line)| line);
        (line, column.chars().count() + 1)
    });
    match position {
        Some((line, column)) => format!(
            "could not parse settings file {}: {} at line {line} column {column}",
            path.display(),
            error.message()
        )
        .into(),
        None => format!(
            "could not parse settings file {}: {}",
            path.display(),
            error.message()
        )
        .into(),
    }
}

fn settings_from_document(
    path: &Path,
    document: &DocumentMut,
) -> Result<Settings, Box<dyn std::error::Error>> {
    let disabled_providers = string_array(path, document, "disabled_providers")?;
    let sentry_url = match document.get("sentry_url") {
        None => None,
        Some(item) => Some(
            item.as_str()
                .ok_or_else(|| invalid_setting(path, "sentry_url", "a string"))?
                .to_owned(),
        ),
    };
    Ok(Settings {
        disabled_providers,
        sentry_url,
        local: None,
    })
}

fn string_array(
    path: &Path,
    document: &DocumentMut,
    key: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    match document.get(key) {
        None => Ok(Vec::new()),
        Some(item) => item
            .as_array()
            .ok_or_else(|| invalid_setting(path, key, "an array of strings"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid_setting(path, key, "an array of strings"))
            })
            .collect(),
    }
}

fn invalid_setting(path: &Path, key: &str, expected: &str) -> Box<dyn std::error::Error> {
    format!(
        "could not parse settings file {}: `{key}` must be {expected}",
        path.display()
    )
    .into()
}

fn retain_recognized_settings(document: &mut DocumentMut) {
    document
        .as_table_mut()
        .retain(|key, _| matches!(key, "disabled_providers" | "sentry_url"));
}

fn set_provider_listed(document: &mut DocumentMut, key: &str, name: &str, listed: bool) {
    let array = match document.get_mut(key) {
        Some(Item::Value(Value::Array(array))) => array,
        Some(_) => unreachable!("settings were validated before mutation"),
        // Nothing to remove from, and no reason to create an empty array.
        None if !listed => return,
        None => {
            document[key] = value(Array::new());
            document[key]
                .as_array_mut()
                .expect("new setting is an array")
        }
    };
    if listed {
        if !array.iter().any(|value| value.as_str() == Some(name)) {
            array.push(name);
        }
    } else {
        loop {
            let index = array.iter().position(|value| value.as_str() == Some(name));
            let Some(index) = index else { break };
            remove_array_value_preserving_previous_decor(array, index);
        }
    }
}

fn remove_array_value_preserving_previous_decor(array: &mut Array, index: usize) {
    // toml_edit stores the text following one array entry (including its
    // inline comment) as the next entry's prefix. Transfer the removed entry's
    // prefix forward so comments belonging to surviving entries stay put.
    let prefix = array
        .get(index)
        .and_then(|value| value.decor().prefix().cloned())
        .unwrap_or_default();
    array.remove(index);
    if let Some(next) = array.get_mut(index) {
        next.decor_mut().set_prefix(prefix);
    } else if !array.is_empty() {
        array.set_trailing_comma(true);
        array.set_trailing(prefix);
    }
}

fn set_string(document: &mut DocumentMut, key: &str, text: String) {
    match document.get_mut(key) {
        Some(Item::Value(old @ Value::String(_))) => {
            let decor = old.decor().clone();
            let mut new = Value::from(text);
            *new.decor_mut() = decor;
            *old = new;
        }
        Some(_) => unreachable!("settings were validated before mutation"),
        None => document[key] = value(text),
    }
}

fn write_settings(path: &Path, document: &DocumentMut) -> Result<(), Box<dyn std::error::Error>> {
    write_atomic(path, |file| file.write_all(document.to_string().as_bytes())).map_err(|error| {
        format!("could not write settings file {}: {error}", path.display()).into()
    })
}

fn write_credentials(
    path: &Path,
    credentials: &Credentials,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(&credentials.values)?;
    write_atomic(path, |file| file.write_all(&bytes)).map_err(|error| {
        format!(
            "could not write credentials file {}: {error}",
            path.display()
        )
        .into()
    })
}

fn write_atomic(
    path: &Path,
    write_body: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::Builder::new().prefix(".foac-").tempfile_in(dir)?;

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
    fn store_paths_prefer_xdg() {
        assert_eq!(
            store_path(Some("/tmp/xdg"), Some(Path::new("/home/u")), "config.toml").unwrap(),
            PathBuf::from("/tmp/xdg/foac/config.toml")
        );
        assert_eq!(
            store_path(
                Some("/tmp/xdg"),
                Some(Path::new("/home/u")),
                "credentials.json"
            )
            .unwrap(),
            PathBuf::from("/tmp/xdg/foac/credentials.json")
        );
    }

    #[test]
    fn store_paths_fall_back_to_home() {
        assert_eq!(
            store_path(None, Some(Path::new("/home/u")), "config.toml").unwrap(),
            PathBuf::from("/home/u/.config/foac/config.toml")
        );
    }

    #[test]
    fn store_paths_treat_empty_xdg_as_unset() {
        assert_eq!(
            store_path(Some(""), Some(Path::new("/home/u")), "credentials.json").unwrap(),
            PathBuf::from("/home/u/.config/foac/credentials.json")
        );
    }

    #[test]
    fn store_path_is_none_without_home_or_xdg() {
        assert_eq!(store_path(None, None, "config.toml"), None);
    }

    #[test]
    fn settings_mutations_match_comment_preserving_golden_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let input = concat!(
            "# foac settings\n",
            "# providers leading comment\n",
            "disabled_providers = [\n",
            "  \"linear\", # keep linear\n",
            "  \"github\", # remove github\n",
            "  \"slack\", # keep slack\n",
            "] # providers inline comment\n",
            "# discard with unknown\n",
            "unknown = \"value\" # discard inline\n",
            "# sentry leading comment\n",
            "sentry_url = \"https://sentry.example.com\" # sentry inline comment\n",
            "# trailing document comment\n",
        );
        std::fs::write(&path, input).unwrap();

        set_enabled_at(&path, "github", true).unwrap();
        set_sentry_url_at(&path, Some("https://sentry.changed.example.com".into())).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "# foac settings\n",
                "# providers leading comment\n",
                "disabled_providers = [\n",
                "  \"linear\", # keep linear\n",
                "  \"slack\", # keep slack\n",
                "] # providers inline comment\n",
                "# sentry leading comment\n",
                "sentry_url = \"https://sentry.changed.example.com\" # sentry inline comment\n",
                "# trailing document comment\n",
            )
        );
    }

    #[test]
    fn credential_updates_are_pretty_and_slack_tokens_are_independent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        set_credentials_at(
            &path,
            &[
                (Credential::Linear, "linear-token"),
                (Credential::Github, "github-token"),
                (Credential::Sentry, "sentry-token"),
                (Credential::SlackBot, "xoxb-bot"),
                (Credential::SlackUser, "xoxp-user"),
            ],
        )
        .unwrap();
        assert!(delete_credentials_at(&path, &[Credential::SlackBot]).unwrap());

        let credentials = load_credentials_from(&path).unwrap();
        assert_eq!(credentials.get(Credential::SlackUser), Some("xoxp-user"));
        assert_eq!(credentials.get(Credential::SlackBot), None);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "{\n",
                "  \"github\": \"github-token\",\n",
                "  \"linear\": \"linear-token\",\n",
                "  \"sentry\": \"sentry-token\",\n",
                "  \"slack_user\": \"xoxp-user\"\n",
                "}"
            )
        );

        assert!(delete_credentials_at(&path, &[Credential::SlackUser]).unwrap());
        assert_eq!(
            load_credentials_from(&path)
                .unwrap()
                .get(Credential::SlackUser),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writer_creates_file_with_private_mode_before_writing() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "foac-provider-private-create-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("credentials.json");

        write_atomic(&path, |file| {
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
    fn atomic_writer_atomically_replaces_existing_file() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "foac-provider-private-existing-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("credentials.json");
        std::fs::write(&path, b"old secret").unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();

        write_atomic(&path, |file| {
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
    fn atomic_writer_keeps_existing_file_on_pre_replace_failure() {
        let dir = std::env::temp_dir().join(format!(
            "foac-provider-private-failure-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("credentials.json");
        std::fs::write(&path, b"old secret").unwrap();

        let error = write_atomic(&path, |file| {
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
    fn stores_handle_missing_malformed_and_unreadable_files_independently() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("config.toml");
        let credentials = dir.path().join("credentials.json");
        assert_eq!(load_settings_from(&settings).unwrap(), Settings::default());
        assert_eq!(
            load_credentials_from(&credentials).unwrap(),
            Credentials::default()
        );

        std::fs::write(&settings, "unknown = \"sensitive-setting-not-for-errors").unwrap();
        std::fs::write(
            &credentials,
            br#"{"github":"sensitive-token-not-for-errors","linear":42}"#,
        )
        .unwrap();
        let settings_error = load_settings_from(&settings).unwrap_err().to_string();
        let credentials_error = load_credentials_from(&credentials).unwrap_err().to_string();
        assert!(settings_error.contains(&settings.display().to_string()));
        assert!(!settings_error.contains(&credentials.display().to_string()));
        assert!(!settings_error.contains("sensitive-setting-not-for-errors"));
        assert!(credentials_error.contains(&credentials.display().to_string()));
        assert!(!credentials_error.contains("sensitive-token-not-for-errors"));

        std::fs::remove_file(&settings).unwrap();
        std::fs::create_dir(&settings).unwrap();
        let error = load_settings_from(&settings).unwrap_err().to_string();
        assert!(error.contains("could not read settings file"));
        assert!(error.contains(&settings.display().to_string()));
    }

    #[test]
    fn each_store_can_mutate_while_the_other_is_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("config.toml");
        let credentials = dir.path().join("credentials.json");
        let malformed_settings = b"disabled_providers = [";
        std::fs::write(&settings, malformed_settings).unwrap();

        set_credentials_at(&credentials, &[(Credential::Github, "token")]).unwrap();
        assert_eq!(std::fs::read(&settings).unwrap(), malformed_settings);

        std::fs::write(&settings, "# settings\ndisabled_providers = []\n").unwrap();
        let malformed_credentials = br#"{"github":"secret","linear":42}"#;
        std::fs::write(&credentials, malformed_credentials).unwrap();
        set_enabled_at(&settings, "github", false).unwrap();
        assert_eq!(std::fs::read(&credentials).unwrap(), malformed_credentials);
    }

    #[test]
    fn local_settings_override_global_toggles() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("config.toml");
        std::fs::write(&global, "disabled_providers = [\"github\", \"sentry\"]\n").unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(
            project.join(LOCAL_SETTINGS_FILE),
            concat!(
                "# per-folder toggles\n",
                "enabled_providers = [\"github\", \"slack\"]\n",
                "disabled_providers = [\"linear\", \"slack\"]\n",
            ),
        )
        .unwrap();

        let mut settings = load_settings_from(&global).unwrap();
        settings.local = load_local_settings(&project).unwrap();

        assert!(settings.enabled("github")); // local enable beats global disable
        assert!(!settings.enabled("linear")); // local disable beats global default
        assert!(settings.enabled("slack")); // listed in both locally: enabled wins
        assert!(!settings.enabled("sentry")); // untouched locally: global applies
    }

    #[test]
    fn nearest_local_settings_file_wins() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        let grandchild = child.join("grandchild");
        std::fs::create_dir_all(&grandchild).unwrap();
        std::fs::write(
            dir.path().join(LOCAL_SETTINGS_FILE),
            "disabled_providers = [\"github\"]\n",
        )
        .unwrap();
        std::fs::write(
            child.join(LOCAL_SETTINGS_FILE),
            "disabled_providers = [\"linear\"]\n",
        )
        .unwrap();

        let local = load_local_settings(&grandchild).unwrap().unwrap();
        assert_eq!(local.path, child.join(LOCAL_SETTINGS_FILE));
        assert_eq!(local.disabled_providers, vec!["linear"]);

        let local = load_local_settings(dir.path()).unwrap().unwrap();
        assert_eq!(local.path, dir.path().join(LOCAL_SETTINGS_FILE));
        assert_eq!(local.disabled_providers, vec!["github"]);
    }

    #[test]
    fn local_mutations_edit_the_nearest_file_and_preserve_comments() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let path = dir.path().join(LOCAL_SETTINGS_FILE);
        let input = concat!(
            "# project toggles\n",
            "enabled_providers = [\n",
            "  \"linear\", # keep linear\n",
            "  \"github\", # remove github\n",
            "]\n",
            "disabled_providers = [\"slack\"]\n",
            "unknown = \"value\"\n",
        );
        std::fs::write(&path, input).unwrap();

        let target = nearest_local_settings_path(&sub);
        assert_eq!(target, path);
        set_enabled_local_at(&target, "github", false).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "# project toggles\n",
                "enabled_providers = [\n",
                "  \"linear\", # keep linear\n",
                // The removed entry's leading whitespace becomes the array's
                // trailing decor, hence the indented bracket.
                "  ]\n",
                "disabled_providers = [\"slack\", \"github\"]\n",
            )
        );
    }

    #[test]
    fn local_mutations_create_a_minimal_file_when_none_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = nearest_local_settings_path(dir.path());
        assert_eq!(path, dir.path().join(LOCAL_SETTINGS_FILE));

        set_enabled_local_at(&path, "github", false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "disabled_providers = [\"github\"]\n"
        );

        set_enabled_local_at(&path, "github", true).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "disabled_providers = []\nenabled_providers = [\"github\"]\n"
        );
    }

    #[test]
    fn ensure_enabled_points_at_the_local_settings_file() {
        let settings = Settings {
            disabled_providers: Vec::new(),
            sentry_url: None,
            local: Some(LocalSettings {
                path: PathBuf::from("/project/.foac.toml"),
                enabled_providers: Vec::new(),
                disabled_providers: vec!["github".into()],
            }),
        };
        let error = ensure_enabled(&settings, "github").unwrap_err().to_string();
        assert_eq!(
            error,
            "github is disabled by /project/.foac.toml; run `foac provider enable github --local` to enable it"
        );
        assert!(ensure_enabled(&settings, "linear").is_ok());
    }

    #[test]
    fn malformed_local_settings_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LOCAL_SETTINGS_FILE);
        std::fs::write(&path, "enabled_providers = \"sensitive-not-for-errors\"").unwrap();
        let error = load_local_settings(dir.path()).unwrap_err().to_string();
        assert!(error.contains(&path.display().to_string()));
        assert!(error.contains("`enabled_providers` must be an array of strings"));
        assert!(!error.contains("sensitive-not-for-errors"));
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
