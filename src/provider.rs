use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde_json::json;
use toml_edit::{Array, DocumentMut, Item, Value, value};

use crate::auth::Provider;

pub const PROVIDERS: [&str; 8] = [
    "confluence",
    "github",
    "jira",
    "linear",
    "neon",
    "sentry",
    "slack",
    "vercel",
];

/// Per-folder settings file, looked up from the working directory to `/`;
/// the nearest file wins and overrides the global toggles.
pub const LOCAL_SETTINGS_FILE: &str = ".foac.toml";

/// The instance an unnamed login creates and unqualified commands use.
pub const DEFAULT_INSTANCE: &str = "default";

#[derive(Subcommand)]
pub enum Cmd {
    /// List providers with their enabled, authenticated, and skill state
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
    /// Per-provider default instance from the `[defaults]` table.
    defaults: BTreeMap<String, String>,
    local: Option<LocalSettings>,
}

/// Toggles from the nearest per-folder settings file; a provider listed in
/// both arrays is enabled.
#[derive(Debug, PartialEq, Eq)]
struct LocalSettings {
    path: PathBuf,
    enabled_providers: Vec<String>,
    disabled_providers: Vec<String>,
    defaults: BTreeMap<String, String>,
}

impl Settings {
    pub fn enabled(&self, name: &str) -> bool {
        match &self.local {
            Some(local) if local.enabled_providers.iter().any(|p| p == name) => true,
            Some(local) if local.disabled_providers.iter().any(|p| p == name) => false,
            _ => !self.disabled_providers.iter().any(|p| p == name),
        }
    }

    /// An instance is active iff its provider is enabled as a whole and the
    /// qualified `provider@instance` name is not disabled.
    pub fn instance_enabled(&self, provider: &str, instance: &str) -> bool {
        self.enabled(provider) && self.enabled(&qualified(provider, instance))
    }

    /// The instance unqualified commands use: nearest `.foac.toml`
    /// `[defaults]`, then the global one, then [`DEFAULT_INSTANCE`].
    pub fn default_instance(&self, provider: &str) -> &str {
        self.local
            .as_ref()
            .and_then(|local| local.defaults.get(provider))
            .or_else(|| self.defaults.get(provider))
            .map_or(DEFAULT_INSTANCE, String::as_str)
    }

    fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_providers.retain(|p| p != name);
        if !enabled {
            self.disabled_providers.push(name.to_owned());
        }
    }
}

/// The qualified toggle/status name for an instance, `provider@instance`.
pub(crate) fn qualified(provider: &str, instance: &str) -> String {
    format!("{provider}@{instance}")
}

/// Validate an instance name: lowercase letters, digits, `-`, `_`.
pub fn normalize_instance(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let name = name.trim();
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && name.starts_with(|c: char| c.is_ascii_alphanumeric());
    if !valid {
        return Err(format!(
            "invalid instance name `{name}`: use lowercase letters, digits, `-`, and `_`, starting with a letter or digit"
        )
        .into());
    }
    Ok(name.to_owned())
}

/// The instance a provider invocation targets:
/// `--instance` flag > `[defaults]` > `default`.
pub fn resolve_instance(
    provider: &str,
    flag: Option<&str>,
    settings: &Settings,
) -> Result<String, Box<dyn std::error::Error>> {
    match flag {
        Some(name) => normalize_instance(name),
        None => Ok(settings.default_instance(provider).to_owned()),
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Credential {
    Linear,
    Github,
    Neon,
    Sentry,
    /// The base URL of a self-hosted Sentry, stored with the instance's token.
    SentryUrl,
    SlackBot,
    SlackUser,
    Vercel,
    // Atlassian credentials are vendor-level: Jira and Confluence share the
    // same host, email, and API token.
    AtlassianHost,
    AtlassianEmail,
    AtlassianToken,
}

impl Credential {
    /// The credentials-file top-level key; Jira and Confluence share the
    /// `atlassian` vendor.
    pub(crate) fn vendor(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Github => "github",
            Self::Neon => "neon",
            Self::Sentry | Self::SentryUrl => "sentry",
            Self::SlackBot | Self::SlackUser => "slack",
            Self::Vercel => "vercel",
            Self::AtlassianHost | Self::AtlassianEmail | Self::AtlassianToken => "atlassian",
        }
    }

    /// The field key inside an instance record.
    fn field(self) -> &'static str {
        match self {
            Self::Linear | Self::Github | Self::Neon | Self::Sentry | Self::Vercel => "token",
            Self::SentryUrl => "url",
            Self::SlackBot => "bot_token",
            Self::SlackUser => "user_token",
            Self::AtlassianHost => "host",
            Self::AtlassianEmail => "email",
            Self::AtlassianToken => "token",
        }
    }
}

/// Stored credentials: vendor -> instance -> field -> value.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Credentials {
    values: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>,
}

impl Credentials {
    pub(crate) fn get(&self, credential: Credential, instance: &str) -> Option<&str> {
        self.values
            .get(credential.vendor())?
            .get(instance)?
            .get(credential.field())
            .map(String::as_str)
    }

    /// The instance names stored for a vendor, in sorted order.
    pub(crate) fn instances(&self, vendor: &str) -> Vec<String> {
        self.values
            .get(vendor)
            .map(|instances| instances.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn set(&mut self, credential: Credential, instance: &str, token: String) {
        self.values
            .entry(credential.vendor().to_owned())
            .or_default()
            .entry(instance.to_owned())
            .or_default()
            .insert(credential.field().to_owned(), token);
    }

    fn remove(&mut self, credential: Credential, instance: &str) -> bool {
        let Some(instances) = self.values.get_mut(credential.vendor()) else {
            return false;
        };
        let Some(fields) = instances.get_mut(instance) else {
            return false;
        };
        let removed = fields.remove(credential.field()).is_some();
        // Drop emptied records so stored instances stay enumerable.
        if fields.is_empty() {
            instances.remove(instance);
        }
        if instances.is_empty() {
            self.values.remove(credential.vendor());
        }
        removed
    }
}

pub struct SettingsStore;

impl SettingsStore {
    pub fn load(&self) -> Result<Settings, Box<dyn std::error::Error>> {
        let mut settings = self.load_global()?;
        settings.local = local_settings_for_cwd()?;
        Ok(settings)
    }

    /// The global settings alone, never reading a per-folder `.foac.toml`;
    /// skills are installed machine-wide, so `skill install` must not depend
    /// on the working directory (a local toggle or a malformed local file
    /// must not affect it).
    pub fn load_global(&self) -> Result<Settings, Box<dyn std::error::Error>> {
        let path = settings_path().ok_or("could not determine the settings path")?;
        load_settings_from(&path)
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
        instance: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = credentials_path().ok_or("could not determine the credentials path")?;
        set_credentials_at(&path, credentials, instance)
    }

    pub(crate) fn delete_many(
        &self,
        credentials: &[Credential],
        instance: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let path = credentials_path().ok_or("could not determine the credentials path")?;
        delete_credentials_at(&path, credentials, instance)
    }
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let instance = instance.as_deref().map(normalize_instance).transpose()?;
    match cmd {
        Cmd::List => {
            crate::output::print(&statuses(&SettingsStore.load()?), format);
            Ok(())
        }
        Cmd::Enable { provider, local } => set_enabled(provider, instance, true, local, format),
        Cmd::Disable { provider, local } => set_enabled(provider, instance, false, local, format),
    }
}

/// The vendor whose credentials authenticate a provider; Jira and Confluence
/// share the Atlassian vendor.
pub(crate) fn provider_vendor(provider: &str) -> &'static str {
    match provider {
        "jira" | "confluence" => "atlassian",
        "github" => "github",
        "linear" => "linear",
        "neon" => "neon",
        "sentry" => "sentry",
        "slack" => "slack",
        "vercel" => "vercel",
        _ => unreachable!("unknown provider"),
    }
}

fn statuses(settings: &Settings) -> serde_json::Value {
    statuses_with(
        settings,
        authenticated,
        &crate::update::installed_skill_providers(),
        &CredentialStore.load().unwrap_or_default(),
    )
}

fn statuses_with(
    settings: &Settings,
    authenticated: impl Fn(&str) -> bool,
    installed_skills: &[&str],
    credentials: &Credentials,
) -> serde_json::Value {
    serde_json::Value::Object(
        PROVIDERS
            .iter()
            .flat_map(|name| {
                let provider = std::iter::once((
                    name.to_string(),
                    json!({
                        "enabled": settings.enabled(name),
                        "authenticated": authenticated(name),
                        "skill_installed": installed_skills.contains(name),
                    }),
                ));
                // One row per stored named instance; the bare row is the
                // default instance.
                let named = credentials
                    .instances(provider_vendor(name))
                    .into_iter()
                    .filter(|instance| instance != DEFAULT_INSTANCE)
                    .map(|instance| {
                        (
                            qualified(name, &instance),
                            json!({
                                "enabled": settings.instance_enabled(name, &instance),
                                "authenticated": true,
                                "skill_installed": installed_skills.contains(name),
                            }),
                        )
                    })
                    .collect::<Vec<_>>();
                provider.chain(named)
            })
            .collect(),
    )
}

/// Whether a credential resolves for the provider; no network validation.
fn authenticated(name: &str) -> bool {
    match name {
        "confluence" => crate::confluence::authenticated(),
        "github" => crate::github::authenticated(),
        "jira" => crate::jira::authenticated(),
        "linear" => crate::linear::authenticated(),
        "neon" => crate::neon::authenticated(),
        "sentry" => crate::sentry::authenticated(),
        "slack" => crate::slack::authenticated(),
        "vercel" => crate::vercel::authenticated(),
        _ => false,
    }
}

fn set_enabled(
    provider: Provider,
    instance: Option<String>,
    enabled: bool,
    local: bool,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    // Bare toggles the whole provider; --instance toggles one instance.
    let name = match &instance {
        Some(instance) => qualified(provider.as_str(), instance),
        None => provider.as_str().to_owned(),
    };
    let settings = if local {
        SettingsStore.set_enabled_local(&name, enabled)?
    } else {
        SettingsStore.set_enabled(&name, enabled)?
    };
    crate::output::print_highlighting(&statuses(&settings), format, &name);
    Ok(())
}

pub fn ensure_enabled(
    settings: &Settings,
    name: &str,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !settings.enabled(name) {
        return Err(match &settings.local {
            Some(local) if local.disabled_providers.iter().any(|p| p == name) => format!(
                "{name} is disabled by {}; run `foac provider enable {name} --local` to enable it",
                local.path.display()
            ),
            _ => format!("{name} is disabled; run `foac provider enable {name}` to enable it"),
        }
        .into());
    }
    let qualified = qualified(name, instance);
    if settings.enabled(&qualified) {
        return Ok(());
    }
    Err(match &settings.local {
        Some(local) if local.disabled_providers.iter().any(|p| p == &qualified) => format!(
            "{qualified} is disabled by {}; run `foac provider enable {name} --instance {instance} --local` to enable it",
            local.path.display()
        ),
        _ => format!(
            "{qualified} is disabled; run `foac provider enable {name} --instance {instance}` to enable it"
        ),
    }
    .into())
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
            defaults: defaults_table(&path, &document)?,
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
        .retain(|key, _| matches!(key, "enabled_providers" | "disabled_providers" | "defaults"));
    set_provider_listed(&mut document, "enabled_providers", name, enabled);
    set_provider_listed(&mut document, "disabled_providers", name, !enabled);
    write_settings(path, &document)
}

fn load_credentials_from(path: &Path) -> Result<Credentials, Box<dyn std::error::Error>> {
    let Some(bytes) = read_file(path, "credentials")? else {
        return Ok(Credentials::default());
    };
    let raw: BTreeMap<String, serde_json::Value> =
        serde_json::from_slice(&bytes).map_err(|error| {
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
    let mut values = BTreeMap::new();
    for (vendor, item) in raw {
        // Skip entries that are not instance maps.
        if item.is_string() {
            continue;
        }
        let instances = serde_json::from_value(item).map_err(|_| {
            format!(
                "could not parse credentials file {}: invalid credential data under `{vendor}`",
                path.display()
            )
        })?;
        values.insert(vendor, instances);
    }
    Ok(Credentials { values })
}

fn set_credentials_at(
    path: &Path,
    updates: &[(Credential, &str)],
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut credentials = load_credentials_from(path)?;
    for (credential, token) in updates {
        credentials.set(*credential, instance, (*token).to_owned());
    }
    write_credentials(path, &credentials)
}

fn delete_credentials_at(
    path: &Path,
    removals: &[Credential],
    instance: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut credentials = load_credentials_from(path)?;
    let mut removed = false;
    for credential in removals {
        removed = credentials.remove(*credential, instance) || removed;
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
    Ok(Settings {
        disabled_providers: string_array(path, document, "disabled_providers")?,
        defaults: defaults_table(path, document)?,
        local: None,
    })
}

/// The `[defaults]` table: provider name -> instance name.
fn defaults_table(
    path: &Path,
    document: &DocumentMut,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let Some(item) = document.get("defaults") else {
        return Ok(BTreeMap::new());
    };
    let table = item
        .as_table_like()
        .ok_or_else(|| invalid_setting(path, "defaults", "a table of instance names"))?;
    let mut defaults = BTreeMap::new();
    for (provider, value) in table.iter() {
        let name = value
            .as_str()
            .ok_or_else(|| invalid_setting(path, "defaults", "a table of instance names"))?;
        let instance = normalize_instance(name).map_err(|error| {
            format!(
                "could not parse settings file {}: `defaults.{provider}`: {error}",
                path.display()
            )
        })?;
        defaults.insert(provider.to_owned(), instance);
    }
    Ok(defaults)
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
        .retain(|key, _| matches!(key, "disabled_providers" | "defaults"));
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
            "  \"slack@workb\", # keep the workb instance off\n",
            "] # providers inline comment\n",
            "# discard with unknown\n",
            "unknown = \"value\" # discard inline\n",
            "# defaults leading comment\n",
            "[defaults]\n",
            "slack = \"worka\" # defaults inline comment\n",
        );
        std::fs::write(&path, input).unwrap();

        set_enabled_at(&path, "github", true).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "# foac settings\n",
                "# providers leading comment\n",
                "disabled_providers = [\n",
                "  \"linear\", # keep linear\n",
                "  \"slack@workb\", # keep the workb instance off\n",
                "] # providers inline comment\n",
                "# defaults leading comment\n",
                "[defaults]\n",
                "slack = \"worka\" # defaults inline comment\n",
            )
        );
    }

    #[test]
    fn credential_updates_are_pretty_and_nested_by_instance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        set_credentials_at(
            &path,
            &[
                (Credential::Linear, "linear-token"),
                (Credential::Github, "github-token"),
                (Credential::SlackBot, "xoxb-bot"),
                (Credential::SlackUser, "xoxp-user"),
            ],
            DEFAULT_INSTANCE,
        )
        .unwrap();
        set_credentials_at(&path, &[(Credential::SlackBot, "xoxb-workb")], "workb").unwrap();
        assert!(delete_credentials_at(&path, &[Credential::SlackBot], DEFAULT_INSTANCE).unwrap());

        let credentials = load_credentials_from(&path).unwrap();
        assert_eq!(
            credentials.get(Credential::SlackUser, DEFAULT_INSTANCE),
            Some("xoxp-user")
        );
        assert_eq!(
            credentials.get(Credential::SlackBot, DEFAULT_INSTANCE),
            None
        );
        assert_eq!(
            credentials.get(Credential::SlackBot, "workb"),
            Some("xoxb-workb")
        );
        assert_eq!(credentials.instances("slack"), vec!["default", "workb"]);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            concat!(
                "{\n",
                "  \"github\": {\n",
                "    \"default\": {\n",
                "      \"token\": \"github-token\"\n",
                "    }\n",
                "  },\n",
                "  \"linear\": {\n",
                "    \"default\": {\n",
                "      \"token\": \"linear-token\"\n",
                "    }\n",
                "  },\n",
                "  \"slack\": {\n",
                "    \"default\": {\n",
                "      \"user_token\": \"xoxp-user\"\n",
                "    },\n",
                "    \"workb\": {\n",
                "      \"bot_token\": \"xoxb-workb\"\n",
                "    }\n",
                "  }\n",
                "}"
            )
        );

        // Removing an instance's last field drops the whole record.
        assert!(delete_credentials_at(&path, &[Credential::SlackUser], DEFAULT_INSTANCE).unwrap());
        let credentials = load_credentials_from(&path).unwrap();
        assert_eq!(
            credentials.get(Credential::SlackUser, DEFAULT_INSTANCE),
            None
        );
        assert_eq!(credentials.instances("slack"), vec!["workb"]);
    }

    #[test]
    fn non_instance_credential_entries_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        std::fs::write(
            &path,
            br#"{"github":"stray-token","slack":{"default":{"bot_token":"xoxb-bot"}}}"#,
        )
        .unwrap();
        let credentials = load_credentials_from(&path).unwrap();
        assert_eq!(credentials.get(Credential::Github, DEFAULT_INSTANCE), None);
        assert_eq!(
            credentials.get(Credential::SlackBot, DEFAULT_INSTANCE),
            Some("xoxb-bot")
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

        set_credentials_at(
            &credentials,
            &[(Credential::Github, "token")],
            DEFAULT_INSTANCE,
        )
        .unwrap();
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

        // Without the local overrides (the `load_global` view), only the
        // global toggles apply.
        settings.local = None;
        assert!(!settings.enabled("github"));
        assert!(settings.enabled("linear"));
        assert!(settings.enabled("slack"));
        assert!(!settings.enabled("sentry"));
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
            "disabled_providers = [\"slack@workb\"]\n",
            "unknown = \"value\"\n",
            "[defaults]\n",
            "slack = \"worka\"\n",
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
                "disabled_providers = [\"slack@workb\", \"github\"]\n",
                "[defaults]\n",
                "slack = \"worka\"\n",
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
            defaults: BTreeMap::new(),
            local: Some(LocalSettings {
                path: PathBuf::from("/project/.foac.toml"),
                enabled_providers: Vec::new(),
                disabled_providers: vec!["github".into(), "slack@workb".into()],
                defaults: BTreeMap::new(),
            }),
        };
        let error = ensure_enabled(&settings, "github", DEFAULT_INSTANCE)
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "github is disabled by /project/.foac.toml; run `foac provider enable github --local` to enable it"
        );
        let error = ensure_enabled(&settings, "slack", "workb")
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "slack@workb is disabled by /project/.foac.toml; run `foac provider enable slack --instance workb --local` to enable it"
        );
        assert!(ensure_enabled(&settings, "linear", DEFAULT_INSTANCE).is_ok());
        assert!(ensure_enabled(&settings, "slack", DEFAULT_INSTANCE).is_ok());
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
        assert!(ensure_enabled(&settings, "github", DEFAULT_INSTANCE).is_ok());
        settings.set_enabled("github", false);
        let error = ensure_enabled(&settings, "github", DEFAULT_INSTANCE)
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "github is disabled; run `foac provider enable github` to enable it"
        );

        settings.set_enabled("slack@workb", false);
        assert!(ensure_enabled(&settings, "slack", DEFAULT_INSTANCE).is_ok());
        let error = ensure_enabled(&settings, "slack", "workb")
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "slack@workb is disabled; run `foac provider enable slack --instance workb` to enable it"
        );
    }

    #[test]
    fn instance_toggles_layer_like_provider_toggles() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("config.toml");
        std::fs::write(
            &global,
            "disabled_providers = [\"slack@workb\", \"github\"]\n",
        )
        .unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(
            project.join(LOCAL_SETTINGS_FILE),
            concat!(
                "enabled_providers = [\"slack@workb\"]\n",
                "disabled_providers = [\"slack@worka\"]\n",
            ),
        )
        .unwrap();

        let mut settings = load_settings_from(&global).unwrap();
        settings.local = load_local_settings(&project).unwrap();

        assert!(settings.instance_enabled("slack", "workb")); // local enable beats global disable
        assert!(!settings.instance_enabled("slack", "worka")); // local disable
        assert!(settings.instance_enabled("slack", DEFAULT_INSTANCE)); // untouched
        // A whole-provider disable turns off every instance.
        assert!(!settings.instance_enabled("github", DEFAULT_INSTANCE));
        assert!(!settings.instance_enabled("github", "work"));
    }

    #[test]
    fn default_instance_prefers_local_then_global_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("config.toml");
        std::fs::write(&global, "[defaults]\nslack = \"worka\"\ngithub = \"org\"\n").unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(
            project.join(LOCAL_SETTINGS_FILE),
            "[defaults]\nslack = \"workb\"\n",
        )
        .unwrap();

        let mut settings = load_settings_from(&global).unwrap();
        settings.local = load_local_settings(&project).unwrap();

        assert_eq!(settings.default_instance("slack"), "workb");
        assert_eq!(settings.default_instance("github"), "org");
        assert_eq!(settings.default_instance("linear"), DEFAULT_INSTANCE);

        assert_eq!(
            resolve_instance("slack", Some("workc"), &settings).unwrap(),
            "workc"
        );
        assert_eq!(resolve_instance("slack", None, &settings).unwrap(), "workb");
        assert!(resolve_instance("slack", Some("Bad Name"), &settings).is_err());
    }

    #[test]
    fn malformed_defaults_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[defaults]\nslack = 3\n").unwrap();
        let error = load_settings_from(&path).unwrap_err().to_string();
        assert!(error.contains("`defaults` must be a table of instance names"));

        std::fs::write(&path, "[defaults]\nslack = \"Bad Name\"\n").unwrap();
        let error = load_settings_from(&path).unwrap_err().to_string();
        assert!(error.contains("`defaults.slack`"));
        assert!(error.contains("invalid instance name"));
    }

    #[test]
    fn instance_names_are_validated() {
        assert_eq!(normalize_instance("workb").unwrap(), "workb");
        assert_eq!(normalize_instance(" work-b_2 ").unwrap(), "work-b_2");
        assert_eq!(normalize_instance("default").unwrap(), DEFAULT_INSTANCE);
        for invalid in ["", "Work", "a b", "@work", "-work", "wörk"] {
            assert!(normalize_instance(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn statuses_lists_every_provider_and_stored_named_instances() {
        let mut settings = Settings::default();
        settings.set_enabled("sentry", false);
        settings.set_enabled("slack@workb", false);
        let mut credentials = Credentials::default();
        credentials.set(Credential::SlackBot, "workb", "xoxb-b".into());
        credentials.set(Credential::SlackBot, DEFAULT_INSTANCE, "xoxb-a".into());
        assert_eq!(
            statuses_with(
                &settings,
                |name| name == "github",
                &["linear"],
                &credentials
            ),
            json!({
                "confluence": {"enabled": true, "authenticated": false, "skill_installed": false},
                "github": {"enabled": true, "authenticated": true, "skill_installed": false},
                "jira": {"enabled": true, "authenticated": false, "skill_installed": false},
                "linear": {"enabled": true, "authenticated": false, "skill_installed": true},
                "neon": {"enabled": true, "authenticated": false, "skill_installed": false},
                "sentry": {"enabled": false, "authenticated": false, "skill_installed": false},
                "slack": {"enabled": true, "authenticated": false, "skill_installed": false},
                "slack@workb": {"enabled": false, "authenticated": true, "skill_installed": false},
                "vercel": {"enabled": true, "authenticated": false, "skill_installed": false},
            })
        );
    }
}
