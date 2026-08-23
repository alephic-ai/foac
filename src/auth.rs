use std::fmt;
use std::io::{IsTerminal, Read};
use std::process::Command as ProcessCommand;

use clap::{Args, Subcommand};
use serde_json::{Value, json};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct Cmd {
    #[command(subcommand)]
    command: AuthCmd,
}

#[derive(Subcommand)]
enum AuthCmd {
    /// Check authentication for every provider
    Status,
    /// Configure Linear authentication
    Linear(ProviderCmd),
    /// Configure GitHub authentication
    Github(ProviderCmd),
    /// Configure Sentry authentication
    Sentry(SentryCmd),
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
struct ProviderCmd {
    #[command(subcommand)]
    command: ProviderAction,
}

#[derive(Subcommand)]
enum ProviderAction {
    /// Check authentication for this provider
    Status,
    /// Validate and save a token in foac's config file
    Login,
    /// Remove foac's token from the config file
    Logout,
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
struct SentryCmd {
    #[command(subcommand)]
    command: SentryAction,
}

/// Same shape as [`ProviderAction`], plus the Sentry-only `--host` login flag.
#[derive(Subcommand)]
enum SentryAction {
    /// Check authentication for this provider
    Status,
    /// Validate and save a token in foac's config file
    Login {
        /// Sentry hostname to log in to, skipping the prompt
        #[arg(long)]
        host: Option<String>,
    },
    /// Remove foac's token from the config file
    Logout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum Provider {
    Linear,
    Github,
    Sentry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialSource {
    Environment,
    ConfigFile,
    GhCli,
}

#[derive(Debug)]
pub(crate) enum ValidationError {
    Rejected(String),
    Failed(String),
}

#[derive(Debug)]
enum ResolveError {
    Missing(String),
    Failed(String),
}

struct ResolvedCredential {
    token: String,
    source: CredentialSource,
}

trait SecretStore {
    fn get(&self, provider: Provider) -> Result<Option<String>, String>;
    fn set(&self, provider: Provider, token: &str) -> Result<(), String>;
    fn delete(&self, provider: Provider) -> Result<bool, String>;
}

struct ConfigFileStore;

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    let store = ConfigFileStore;
    match cmd.command {
        AuthCmd::Status => {
            print_all_statuses(&store, format);
            Ok(())
        }
        AuthCmd::Linear(cmd) => run_provider(Provider::Linear, cmd.command, &store, format),
        AuthCmd::Github(cmd) => run_provider(Provider::Github, cmd.command, &store, format),
        AuthCmd::Sentry(cmd) => match cmd.command {
            SentryAction::Status => {
                print_status(Provider::Sentry, &store, format);
                Ok(())
            }
            SentryAction::Login { host } => login(Provider::Sentry, host, &store, format),
            SentryAction::Logout => logout(Provider::Sentry, &store, format),
        },
    }
}

pub(crate) fn linear_token() -> Result<String, Box<dyn std::error::Error>> {
    resolve_stored(
        Provider::Linear,
        environment_token(Provider::Linear),
        &ConfigFileStore,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

pub(crate) fn sentry_token() -> Result<String, Box<dyn std::error::Error>> {
    resolve_stored(
        Provider::Sentry,
        environment_token(Provider::Sentry),
        &ConfigFileStore,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

pub(crate) fn github_token() -> Result<String, Box<dyn std::error::Error>> {
    resolve_github(
        environment_token(Provider::Github),
        &ConfigFileStore,
        github_cli_token,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ValidationError {}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ResolveError {}

impl Provider {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Github => "github",
            Self::Sentry => "sentry",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Github => "GitHub",
            Self::Sentry => "Sentry",
        }
    }

    fn environment_variable(self) -> &'static str {
        match self {
            Self::Linear => "LINEAR_API_KEY",
            Self::Github => "GITHUB_TOKEN",
            Self::Sentry => "SENTRY_AUTH_TOKEN",
        }
    }
}

impl CredentialSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::ConfigFile => "config_file",
            Self::GhCli => "gh_cli",
        }
    }
}

impl SecretStore for ConfigFileStore {
    fn get(&self, provider: Provider) -> Result<Option<String>, String> {
        Ok(crate::provider::load()
            .credential(provider.as_str())
            .filter(|token| !token.is_empty())
            .map(str::to_owned))
    }

    fn set(&self, provider: Provider, token: &str) -> Result<(), String> {
        let mut config = crate::provider::load();
        config.set_credential(provider.as_str(), token.to_owned());
        crate::provider::save(&config).map_err(|error| error.to_string())
    }

    fn delete(&self, provider: Provider) -> Result<bool, String> {
        let mut config = crate::provider::load();
        let removed = config.remove_credential(provider.as_str());
        if removed {
            crate::provider::save(&config).map_err(|error| error.to_string())?;
        }
        Ok(removed)
    }
}

fn run_provider(
    provider: Provider,
    action: ProviderAction,
    store: &dyn SecretStore,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ProviderAction::Status => {
            print_status(provider, store, format);
            Ok(())
        }
        ProviderAction::Login => login(provider, None, store, format),
        ProviderAction::Logout => logout(provider, store, format),
    }
}

fn logout(
    provider: Provider,
    store: &dyn SecretStore,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let removed = store
        .delete(provider)
        .map_err(|error| format!("could not delete {} credential: {error}", provider.as_str()))?;
    let report = logout_report(provider, removed);
    crate::output::print_text(&logout_summary(removed), &report, format);
    Ok(())
}

fn login(
    provider: Provider,
    host: Option<String>,
    store: &dyn SecretStore,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = match (provider, host) {
        (Provider::Sentry, Some(host)) => Some(crate::sentry::normalize_host(&host)),
        (Provider::Sentry, None) => read_sentry_host()?,
        _ => None,
    };
    print_login_help(provider, url.as_deref());
    let token = read_token()?;
    if token.is_empty() {
        return Err("token cannot be empty".into());
    }
    let account = validate_and_store(provider, &token, store, |token| {
        validate(provider, token, url.as_deref())
    })?;
    if let Some(url) = &url {
        let mut config = crate::provider::load();
        config.set_sentry_url(Some(url.clone()));
        crate::provider::save(&config)?;
        if std::env::var("SENTRY_URL").is_ok_and(|url| !url.is_empty()) {
            eprintln!("Warning: SENTRY_URL is set and takes precedence over the stored URL.");
        }
    }
    if environment_token(provider).is_some() {
        eprintln!(
            "Warning: {} is set and takes precedence over the stored credential.",
            provider.environment_variable()
        );
    }
    let report = login_report(provider, account);
    crate::output::print_text(
        &status_summary(provider, &report[provider.as_str()]),
        &report,
        format,
    );
    Ok(())
}

fn print_all_statuses(store: &dyn SecretStore, format: crate::output::Format) {
    let statuses = all_provider_statuses(store);
    if format == crate::output::Format::Table {
        crate::output::print(&flatten_accounts_for_table(&statuses), format);
    } else {
        crate::output::print(&statuses, format);
    }
}

fn print_status(provider: Provider, store: &dyn SecretStore, format: crate::output::Format) {
    let report = keyed_provider_status(provider, store);
    crate::output::print_text(
        &status_summary(provider, &report[provider.as_str()]),
        &report,
        format,
    );
}

fn nest(provider: Provider, body: Value) -> Value {
    json!({ provider.as_str(): body })
}

fn login_report(provider: Provider, account: Value) -> Value {
    nest(
        provider,
        json!({
            "status": "authenticated",
            "source": CredentialSource::ConfigFile.as_str(),
            "account": account,
        }),
    )
}

fn logout_report(provider: Provider, removed: bool) -> Value {
    nest(provider, json!({ "removed": removed }))
}

fn keyed_provider_status(provider: Provider, store: &dyn SecretStore) -> Value {
    nest(provider, provider_status(provider, store))
}

fn flatten_accounts_for_table(statuses: &Value) -> Value {
    let Some(map) = statuses.as_object() else {
        return statuses.clone();
    };
    let mut out = map.clone();
    for provider in [Provider::Linear, Provider::Github, Provider::Sentry] {
        let Some(obj) = out
            .get_mut(provider.as_str())
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        if let Some(account) = obj.get("account").cloned().filter(Value::is_object) {
            obj.insert(
                "account".into(),
                json!(account_identity(provider, &account)),
            );
        }
    }
    Value::Object(out)
}

fn status_summary(provider: Provider, body: &Value) -> String {
    let status = body["status"].as_str().unwrap_or("unknown");
    let line1 = match body["source"].as_str() {
        Some(source) => format!("{status} via {source}"),
        None => status.to_owned(),
    };
    let line2 = if status == "authenticated" {
        body.get("account")
            .map(|account| account_identity(provider, account))
            .filter(|identity| !identity.is_empty())
    } else {
        body["error"]
            .as_str()
            .map(str::to_owned)
            .filter(|error| !error.is_empty())
    };
    match line2 {
        Some(line2) => format!("{line1}\n{line2}\n"),
        None => format!("{line1}\n"),
    }
}

fn logout_summary(removed: bool) -> String {
    if removed {
        "removed stored credential\n".to_owned()
    } else {
        "no stored credential\n".to_owned()
    }
}

fn account_identity(provider: Provider, account: &Value) -> String {
    match provider {
        Provider::Linear => linear_identity(account),
        Provider::Github => github_identity(account),
        Provider::Sentry => sentry_identity(account),
    }
}

fn linear_identity(account: &Value) -> String {
    let name = text_field(account, "name").or_else(|| text_field(account, "displayName"));
    let email = text_field(account, "email");
    let workspace = account
        .get("workspace")
        .and_then(|workspace| text_field(workspace, "name"));
    let person = match (name, email) {
        (Some(name), Some(email)) => format!("{name} <{email}>"),
        (Some(name), None) => name.to_owned(),
        (None, Some(email)) => email.to_owned(),
        (None, None) => String::new(),
    };
    match (person.is_empty(), workspace) {
        (false, Some(workspace)) => format!("{person}  {workspace}"),
        (false, None) => person,
        (true, Some(workspace)) => workspace.to_owned(),
        (true, None) => String::new(),
    }
}

fn github_identity(account: &Value) -> String {
    let login = text_field(account, "login");
    let name = text_field(account, "name");
    match (login, name) {
        (Some(login), Some(name)) if name != login => format!("{login} ({name})"),
        (Some(login), _) => login.to_owned(),
        (None, Some(name)) => name.to_owned(),
        (None, None) => String::new(),
    }
}

fn sentry_identity(account: &Value) -> String {
    let Some(organizations) = account.get("organizations").and_then(Value::as_array) else {
        return String::new();
    };
    organizations
        .iter()
        .filter_map(|organization| {
            text_field(organization, "slug").or_else(|| text_field(organization, "name"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn text_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

fn provider_status(provider: Provider, store: &dyn SecretStore) -> Value {
    let resolved = match provider {
        Provider::Linear | Provider::Sentry => {
            resolve_stored(provider, environment_token(provider), store)
        }
        Provider::Github => resolve_github(environment_token(provider), store, github_cli_token),
    };
    credential_status(resolved, |token| validate(provider, token, None))
}

fn all_provider_statuses(store: &dyn SecretStore) -> Value {
    json!({
        "linear": provider_status(Provider::Linear, store),
        "github": provider_status(Provider::Github, store),
        "sentry": provider_status(Provider::Sentry, store),
    })
}

fn validate_and_store<F>(
    provider: Provider,
    token: &str,
    store: &dyn SecretStore,
    validate: F,
) -> Result<Value, Box<dyn std::error::Error>>
where
    F: FnOnce(&str) -> Result<Value, ValidationError>,
{
    let account = validate(token)?;
    store
        .set(provider, token)
        .map_err(|error| format!("could not store {} credential: {error}", provider.as_str()))?;
    Ok(account)
}

fn credential_status<F>(resolved: Result<ResolvedCredential, ResolveError>, validate: F) -> Value
where
    F: FnOnce(&str) -> Result<Value, ValidationError>,
{
    let credential = match resolved {
        Ok(credential) => credential,
        Err(ResolveError::Missing(error)) => {
            return json!({
                "status": "unauthenticated",
                "source": Value::Null,
                "error": error,
            });
        }
        Err(ResolveError::Failed(error)) => {
            return json!({
                "status": "error",
                "source": Value::Null,
                "error": error,
            });
        }
    };
    match validate(&credential.token) {
        Ok(account) => json!({
            "status": "authenticated",
            "source": credential.source.as_str(),
            "account": account,
        }),
        Err(ValidationError::Rejected(error)) => json!({
            "status": "unauthenticated",
            "source": credential.source.as_str(),
            "error": error,
        }),
        Err(ValidationError::Failed(error)) => json!({
            "status": "error",
            "source": credential.source.as_str(),
            "error": error,
        }),
    }
}

fn resolve_stored(
    provider: Provider,
    environment: Option<String>,
    store: &dyn SecretStore,
) -> Result<ResolvedCredential, ResolveError> {
    if let Some(token) = environment {
        return Ok(ResolvedCredential {
            token,
            source: CredentialSource::Environment,
        });
    }
    match store.get(provider) {
        Ok(Some(token)) => Ok(ResolvedCredential {
            token,
            source: CredentialSource::ConfigFile,
        }),
        Ok(None) => Err(ResolveError::Missing(format!(
            "{} is not set and no {} credential is stored",
            provider.environment_variable(),
            provider.display_name(),
        ))),
        Err(error) => Err(ResolveError::Failed(format!(
            "could not read {} credential from the config file: {error}",
            provider.display_name(),
        ))),
    }
}

fn resolve_github<F>(
    environment: Option<String>,
    store: &dyn SecretStore,
    github_cli: F,
) -> Result<ResolvedCredential, ResolveError>
where
    F: FnOnce() -> Result<Option<String>, String>,
{
    if let Some(token) = environment {
        return Ok(ResolvedCredential {
            token,
            source: CredentialSource::Environment,
        });
    }
    let store_error = match store.get(Provider::Github) {
        Ok(Some(token)) => {
            return Ok(ResolvedCredential {
                token,
                source: CredentialSource::ConfigFile,
            });
        }
        Ok(None) => None,
        Err(error) => Some(error),
    };
    match github_cli() {
        Ok(Some(token)) => Ok(ResolvedCredential {
            token,
            source: CredentialSource::GhCli,
        }),
        Ok(None) => match store_error {
            Some(error) => Err(ResolveError::Failed(format!(
                "could not read GitHub credential from the config file: {error}"
            ))),
            None => Err(ResolveError::Missing(
                "GITHUB_TOKEN is not set, no GitHub credential is stored, and `gh auth token` did not return a token".into(),
            )),
        },
        Err(error) => Err(ResolveError::Failed(match store_error {
            Some(store_error) => format!(
                "could not read GitHub credential from the config file ({store_error}) or GitHub CLI ({error})"
            ),
            None => format!("could not read GitHub CLI credential: {error}"),
        })),
    }
}

fn validate(
    provider: Provider,
    token: &str,
    sentry_url: Option<&str>,
) -> Result<Value, ValidationError> {
    match provider {
        Provider::Linear => crate::linear::auth_identity(token).map(linear_account),
        Provider::Github => crate::github::auth_identity(token).map(github_account),
        Provider::Sentry => crate::sentry::auth_identity(token, sentry_url).map(sentry_account),
    }
}

fn linear_account(identity: Value) -> Value {
    let viewer = &identity["viewer"];
    let workspace = &identity["organization"];
    json!({
        "id": viewer["id"],
        "name": viewer["name"],
        "displayName": viewer["displayName"],
        "email": viewer["email"],
        "workspace": {
            "id": workspace["id"],
            "name": workspace["name"],
            "urlKey": workspace["urlKey"],
        },
    })
}

fn github_account(identity: Value) -> Value {
    json!({
        "id": identity["id"],
        "login": identity["login"],
        "name": identity["name"],
    })
}

fn sentry_account(identity: Value) -> Value {
    let organizations: Vec<Value> = identity
        .as_array()
        .map(|organizations| {
            organizations
                .iter()
                .map(|organization| {
                    json!({
                        "id": organization["id"],
                        "slug": organization["slug"],
                        "name": organization["name"],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "organizations": organizations })
}

fn environment_token(provider: Provider) -> Option<String> {
    std::env::var(provider.environment_variable())
        .ok()
        .filter(|token| !token.is_empty())
}

fn github_cli_token() -> Result<Option<String>, String> {
    let output = match ProcessCommand::new("gh").args(["auth", "token"]).output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !output.status.success() {
        return Ok(None);
    }
    String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())
        .map(|token| {
            let token = token.trim().to_owned();
            (!token.is_empty()).then_some(token)
        })
}

/// Ask which Sentry host to log in to, defaulting to sentry.io. Skipped when
/// stdin is redirected: piped input stays token-only, and the host falls back
/// to `SENTRY_URL`, then the config file, then the default.
fn read_sentry_host() -> Result<Option<String>, Box<dyn std::error::Error>> {
    if !std::io::stdin().is_terminal() {
        return Ok(None);
    }
    eprint!("Sentry host [{}]: ", crate::sentry::DEFAULT_HOST);
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(Some(crate::sentry::normalize_host(&line)))
}

fn read_token() -> Result<String, Box<dyn std::error::Error>> {
    if std::io::stdin().is_terminal() {
        return Ok(rpassword::prompt_password("Token: ")?);
    }
    let mut token = String::new();
    std::io::stdin().read_to_string(&mut token)?;
    Ok(token.trim_end_matches(['\r', '\n']).to_owned())
}

fn print_login_help(provider: Provider, sentry_url: Option<&str>) {
    match provider {
        Provider::Linear => eprintln!(
            "Create a personal API key at https://linear.app/settings/account/security and grant the permissions needed by your foac commands."
        ),
        Provider::Github => eprintln!(
            "Create a fine-grained personal access token at https://github.com/settings/personal-access-tokens/new and grant the repository permissions needed by your foac commands."
        ),
        Provider::Sentry => eprintln!(
            "Create a user auth token at {}/settings/account/api/auth-tokens/ and grant the scopes needed by your foac commands.",
            sentry_url.map_or_else(crate::sentry::base_url, str::to_owned)
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        credentials: RefCell<HashMap<&'static str, String>>,
        get_error: Option<String>,
    }

    impl SecretStore for MemoryStore {
        fn get(&self, provider: Provider) -> Result<Option<String>, String> {
            if let Some(error) = &self.get_error {
                return Err(error.clone());
            }
            Ok(self.credentials.borrow().get(provider.as_str()).cloned())
        }

        fn set(&self, provider: Provider, token: &str) -> Result<(), String> {
            self.credentials
                .borrow_mut()
                .insert(provider.as_str(), token.to_owned());
            Ok(())
        }

        fn delete(&self, provider: Provider) -> Result<bool, String> {
            Ok(self
                .credentials
                .borrow_mut()
                .remove(provider.as_str())
                .is_some())
        }
    }

    #[test]
    fn linear_credentials_prefer_environment_then_secret_store() {
        let store = MemoryStore::default();
        store.set(Provider::Linear, "stored").unwrap();
        let resolved =
            resolve_stored(Provider::Linear, Some("environment".into()), &store).unwrap();
        assert_eq!(resolved.token, "environment");
        assert_eq!(resolved.source, CredentialSource::Environment);

        let resolved = resolve_stored(Provider::Linear, None, &store).unwrap();
        assert_eq!(resolved.token, "stored");
        assert_eq!(resolved.source, CredentialSource::ConfigFile);
    }

    #[test]
    fn github_credentials_fall_back_to_cli_after_secret_store_failure() {
        let store = MemoryStore {
            get_error: Some("store unavailable".into()),
            ..Default::default()
        };
        let resolved = resolve_github(None, &store, || Ok(Some("gh-token".into()))).unwrap();
        assert_eq!(resolved.token, "gh-token");
        assert_eq!(resolved.source, CredentialSource::GhCli);
    }

    #[test]
    fn missing_credentials_are_distinct_from_store_errors() {
        assert!(matches!(
            resolve_stored(Provider::Linear, None, &MemoryStore::default()),
            Err(ResolveError::Missing(_))
        ));
        let Err(sentry) = resolve_stored(Provider::Sentry, None, &MemoryStore::default()) else {
            panic!("missing Sentry credential should not resolve");
        };
        assert_eq!(
            sentry.to_string(),
            "SENTRY_AUTH_TOKEN is not set and no Sentry credential is stored"
        );
        let store = MemoryStore {
            get_error: Some("locked".into()),
            ..Default::default()
        };
        assert!(matches!(
            resolve_stored(Provider::Linear, None, &store),
            Err(ResolveError::Failed(_))
        ));
    }

    #[test]
    fn memory_store_logout_is_idempotent() {
        let store = MemoryStore::default();
        store.set(Provider::Github, "token").unwrap();
        assert!(store.delete(Provider::Github).unwrap());
        assert!(!store.delete(Provider::Github).unwrap());
    }

    #[test]
    fn account_output_keeps_only_safe_identity_fields() {
        let linear = linear_account(json!({
            "viewer": {
                "id": "user-id",
                "name": "User",
                "displayName": "Display",
                "email": "user@example.com",
                "private": "excluded",
            },
            "organization": {
                "id": "workspace-id",
                "name": "Workspace",
                "urlKey": "workspace",
                "private": "excluded",
            }
        }));
        assert_eq!(linear["id"], "user-id");
        assert_eq!(linear["workspace"]["urlKey"], "workspace");
        assert!(linear.get("private").is_none());

        let github = github_account(json!({
            "id": 1,
            "login": "octocat",
            "name": "The Octocat",
            "token": "excluded",
        }));
        assert_eq!(
            github,
            json!({
                "id": 1,
                "login": "octocat",
                "name": "The Octocat",
            })
        );
    }

    #[test]
    fn single_provider_status_nests_under_the_provider_key() {
        let status = keyed_provider_status(Provider::Linear, &MemoryStore::default());
        assert_eq!(status["linear"]["status"], "unauthenticated");
        assert!(status.get("status").is_none());
        assert!(status.get("github").is_none());
    }

    #[test]
    fn login_and_logout_reports_nest_under_the_provider_key() {
        let login = login_report(Provider::Linear, json!({ "id": "user-id" }));
        assert_eq!(
            login,
            json!({
                "linear": {
                    "status": "authenticated",
                    "source": "config_file",
                    "account": { "id": "user-id" },
                }
            })
        );
        let logout = logout_report(Provider::Sentry, false);
        assert_eq!(logout, json!({ "sentry": { "removed": false } }));
    }

    #[test]
    fn account_identity_summarizes_each_provider() {
        let linear = linear_account(json!({
            "viewer": {
                "id": "user-id",
                "name": "Laurent Raufaste",
                "displayName": "laurent",
                "email": "laurent@alephic.com",
            },
            "organization": { "id": "ws", "name": "Alephic", "urlKey": "alephic" },
        }));
        assert_eq!(
            account_identity(Provider::Linear, &linear),
            "Laurent Raufaste <laurent@alephic.com>  Alephic"
        );

        let github = github_account(json!({
            "id": 1,
            "login": "octocat",
            "name": "The Octocat",
        }));
        assert_eq!(
            account_identity(Provider::Github, &github),
            "octocat (The Octocat)"
        );
        assert_eq!(
            account_identity(
                Provider::Github,
                &github_account(json!({ "id": 1, "login": "octocat", "name": "octocat" }))
            ),
            "octocat"
        );

        let sentry = sentry_account(json!([
            { "id": "1", "slug": "acme", "name": "Acme" },
            { "id": "2", "slug": "alephic", "name": "Alephic" },
        ]));
        assert_eq!(account_identity(Provider::Sentry, &sentry), "acme, alephic");
        assert_eq!(
            account_identity(Provider::Sentry, &sentry_account(json!([]))),
            ""
        );
    }

    #[test]
    fn status_summary_is_two_lines_when_there_is_detail() {
        assert_eq!(
            status_summary(
                Provider::Linear,
                &json!({
                    "status": "authenticated",
                    "source": "environment",
                    "account": {
                        "name": "Laurent Raufaste",
                        "email": "laurent@alephic.com",
                        "workspace": { "name": "Alephic" },
                    },
                })
            ),
            "authenticated via environment\nLaurent Raufaste <laurent@alephic.com>  Alephic\n"
        );
        assert_eq!(
            status_summary(
                Provider::Linear,
                &json!({
                    "status": "unauthenticated",
                    "source": Value::Null,
                    "error": "LINEAR_API_KEY is not set and no Linear credential is stored",
                })
            ),
            "unauthenticated\nLINEAR_API_KEY is not set and no Linear credential is stored\n"
        );
        assert_eq!(
            status_summary(
                Provider::Linear,
                &json!({
                    "status": "unauthenticated",
                    "source": "environment",
                    "error": "rejected",
                })
            ),
            "unauthenticated via environment\nrejected\n"
        );
    }

    #[test]
    fn logout_summary_distinguishes_removed_from_missing() {
        assert_eq!(logout_summary(true), "removed stored credential\n");
        assert_eq!(logout_summary(false), "no stored credential\n");
    }

    #[test]
    fn flatten_accounts_replaces_nested_account_with_identity() {
        let statuses = json!({
            "linear": {
                "status": "authenticated",
                "source": "environment",
                "account": {
                    "name": "Laurent Raufaste",
                    "email": "laurent@alephic.com",
                    "workspace": { "name": "Alephic" },
                },
            },
            "github": {
                "status": "unauthenticated",
                "source": Value::Null,
                "error": "missing",
            },
        });
        let table = flatten_accounts_for_table(&statuses);
        assert_eq!(
            table["linear"]["account"],
            "Laurent Raufaste <laurent@alephic.com>  Alephic"
        );
        assert!(table["github"].get("account").is_none());
        assert_eq!(
            statuses["linear"]["account"]["email"],
            "laurent@alephic.com"
        );
    }

    #[test]
    fn status_serializes_missing_rejected_and_failed_credentials() {
        let missing = credential_status(
            Err(ResolveError::Missing("missing".into())),
            |_| unreachable!(),
        );
        assert_eq!(missing["status"], "unauthenticated");
        assert!(missing["source"].is_null());

        let rejected = credential_status(
            Ok(ResolvedCredential {
                token: "bad-token".into(),
                source: CredentialSource::Environment,
            }),
            |_| Err(ValidationError::Rejected("rejected".into())),
        );
        assert_eq!(rejected["status"], "unauthenticated");
        assert_eq!(rejected["source"], "environment");

        let failed = credential_status(
            Ok(ResolvedCredential {
                token: "token".into(),
                source: CredentialSource::ConfigFile,
            }),
            |_| Err(ValidationError::Failed("offline".into())),
        );
        assert_eq!(failed["status"], "error");
        assert_eq!(failed["source"], "config_file");
    }

    #[test]
    fn login_validates_before_replacing_a_stored_credential() {
        let store = MemoryStore::default();
        store.set(Provider::Linear, "existing-token").unwrap();

        let result = validate_and_store(Provider::Linear, "bad-token", &store, |_| {
            Err(ValidationError::Rejected("rejected".into()))
        });
        assert!(result.is_err());
        assert_eq!(
            store.get(Provider::Linear).unwrap().as_deref(),
            Some("existing-token")
        );

        let account = json!({ "id": "user-id" });
        let result = validate_and_store(Provider::Linear, "new-token", &store, |_| {
            Ok(account.clone())
        })
        .unwrap();
        assert_eq!(result, account);
        assert_eq!(
            store.get(Provider::Linear).unwrap().as_deref(),
            Some("new-token")
        );
    }
}
