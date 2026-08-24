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
    /// Configure Jira (Atlassian) authentication
    Jira(JiraCmd),
    /// Configure Slack authentication
    Slack(SlackCmd),
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
    /// Validate and save a token in foac's credentials file
    Login,
    /// Remove foac's token from the credentials file
    Logout,
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
struct SlackCmd {
    #[command(subcommand)]
    command: SlackAction,
}

#[derive(Subcommand)]
enum SlackAction {
    /// Check Slack authentication
    Status,
    /// Validate and save optional bot and user tokens
    ///
    /// Prompts for the bot token, then the user token. Redirected input uses
    /// two lines in the same order; either token may be blank.
    Login,
    /// Remove foac's stored bot and user tokens
    Logout,
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
struct JiraCmd {
    #[command(subcommand)]
    command: JiraAction,
}

#[derive(Subcommand)]
enum JiraAction {
    /// Check authentication for this provider
    Status,
    /// Validate and save the Atlassian host, email, and API token
    ///
    /// Prompts for the host, email, then API token. Redirected input supplies
    /// one line per missing value in the same order; --host and --email skip
    /// their prompt or line. The stored credential is shared with future
    /// Atlassian providers.
    Login {
        /// Atlassian site host like acme.atlassian.net, skipping the prompt
        #[arg(long)]
        host: Option<String>,
        /// Atlassian account email, skipping the prompt
        #[arg(long)]
        email: Option<String>,
    },
    /// Remove foac's stored Atlassian host, email, and API token
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
    /// Validate and save a token in foac's credentials file
    Login {
        /// Sentry hostname to log in to, skipping the prompt
        #[arg(long)]
        host: Option<String>,
    },
    /// Remove foac's token from the credentials file
    Logout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum Provider {
    Linear,
    Github,
    Jira,
    Slack,
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

#[derive(Debug)]
struct ResolvedCredential {
    token: String,
    source: CredentialSource,
}

const SLACK_APP_MANIFEST: &str = r#"{
  "_metadata": {
    "major_version": 1
  },
  "display_information": {
    "name": "foac",
    "description": "Use the foac CLI with Slack."
  },
  "features": {
    "bot_user": {
      "display_name": "foac",
      "always_online": false
    }
  },
  "oauth_config": {
    "scopes": {
      "bot": [
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
      ],
      "user": [
        "search:read"
      ]
    }
  },
  "settings": {
    "org_deploy_enabled": false,
    "socket_mode_enabled": false,
    "token_rotation_enabled": false
  }
}"#;

trait SecretStore {
    fn get(&self, credential: crate::provider::Credential) -> Result<Option<String>, String>;
    fn set_many(&self, credentials: &[(crate::provider::Credential, &str)]) -> Result<(), String>;
    fn delete_many(&self, credentials: &[crate::provider::Credential]) -> Result<bool, String>;
}

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    let store = crate::provider::CredentialStore;
    match cmd.command {
        AuthCmd::Status => {
            print_all_statuses(&store, format);
            Ok(())
        }
        AuthCmd::Linear(cmd) => run_provider(Provider::Linear, cmd.command, &store, format),
        AuthCmd::Github(cmd) => run_provider(Provider::Github, cmd.command, &store, format),
        AuthCmd::Jira(cmd) => match cmd.command {
            JiraAction::Status => {
                print_status(Provider::Jira, &store, format);
                Ok(())
            }
            JiraAction::Login { host, email } => jira_login(host, email, &store, format),
            JiraAction::Logout => logout(Provider::Jira, &store, format),
        },
        AuthCmd::Slack(cmd) => run_slack(cmd.command, &store, format),
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
        &crate::provider::CredentialStore,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

pub(crate) fn sentry_token() -> Result<String, Box<dyn std::error::Error>> {
    resolve_stored(
        Provider::Sentry,
        environment_token(Provider::Sentry),
        &crate::provider::CredentialStore,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

pub(crate) fn slack_token() -> Result<String, Box<dyn std::error::Error>> {
    resolve_slack(
        environment_token(Provider::Slack),
        environment_slack_user_token(),
        &crate::provider::CredentialStore,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

pub(crate) fn slack_user_token() -> Result<String, Box<dyn std::error::Error>> {
    resolve_slack_user(
        environment_slack_user_token(),
        &crate::provider::CredentialStore,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

pub(crate) fn github_token() -> Result<String, Box<dyn std::error::Error>> {
    resolve_github(
        environment_token(Provider::Github),
        &crate::provider::CredentialStore,
        github_cli_token,
    )
    .map(|credential| credential.token)
    .map_err(Into::into)
}

/// Vendor-level Atlassian credentials: every Jira request needs the tenant
/// host, the account email, and the API token.
#[derive(Debug)]
pub(crate) struct JiraCredentials {
    pub(crate) host: String,
    pub(crate) email: String,
    pub(crate) token: String,
}

#[derive(Debug)]
struct ResolvedJira {
    credentials: JiraCredentials,
    /// The token's source; the host and email may come from elsewhere.
    source: CredentialSource,
}

/// Resolve the credentials for a `foac jira` invocation. Each value falls
/// back flag > environment > stored; a missing token is additionally read
/// from redirected stdin so a one-off invocation never puts it in shell
/// history.
pub(crate) fn jira_credentials(
    host_flag: Option<String>,
    email_flag: Option<String>,
) -> Result<JiraCredentials, Box<dyn std::error::Error>> {
    let store = crate::provider::CredentialStore;
    let host = match host_flag {
        Some(host) => Some(host),
        None => jira_part(
            environment("ATLASSIAN_HOST"),
            crate::provider::Credential::AtlassianHost,
            "Atlassian host",
            &store,
        )?
        .map(|(value, _)| value),
    }
    .ok_or("--host or ATLASSIAN_HOST is not set and no Atlassian host is stored; run `foac auth jira login`")?;
    let email = match email_flag {
        Some(email) => Some(email),
        None => jira_part(
            environment("ATLASSIAN_EMAIL"),
            crate::provider::Credential::AtlassianEmail,
            "Atlassian email",
            &store,
        )?
        .map(|(value, _)| value),
    }
    .ok_or("--email or ATLASSIAN_EMAIL is not set and no Atlassian email is stored; run `foac auth jira login`")?;
    let token = match jira_part(
        environment("ATLASSIAN_API_TOKEN"),
        crate::provider::Credential::AtlassianToken,
        "Atlassian API token",
        &store,
    )? {
        Some((token, _)) => token,
        None if !std::io::stdin().is_terminal() => {
            let token = read_token()?;
            if token.is_empty() {
                return Err("the Atlassian API token piped to stdin is empty".into());
            }
            token
        }
        None => {
            return Err(
                "ATLASSIAN_API_TOKEN is not set and no Atlassian API token is stored; pipe the token to stdin or run `foac auth jira login`"
                    .into(),
            );
        }
    };
    Ok(JiraCredentials {
        host: crate::jira::normalize_host(&host)?,
        email,
        token,
    })
}

pub(crate) fn jira_authenticated() -> bool {
    resolve_jira(jira_environment(), &crate::provider::CredentialStore).is_ok()
}

/// The `ATLASSIAN_HOST`, `ATLASSIAN_EMAIL`, and `ATLASSIAN_API_TOKEN`
/// environment values, in that order.
fn jira_environment() -> [Option<String>; 3] {
    [
        environment("ATLASSIAN_HOST"),
        environment("ATLASSIAN_EMAIL"),
        environment("ATLASSIAN_API_TOKEN"),
    ]
}

fn resolve_jira(
    environment: [Option<String>; 3],
    store: &dyn SecretStore,
) -> Result<ResolvedJira, ResolveError> {
    let [host_environment, email_environment, token_environment] = environment;
    let resolve = |environment,
                   variable: &str,
                   credential,
                   display_name: &str|
     -> Result<(String, CredentialSource), ResolveError> {
        jira_part(environment, credential, display_name, store)?.ok_or_else(|| {
            ResolveError::Missing(format!(
                "{variable} is not set and no {display_name} is stored"
            ))
        })
    };
    let (host, _) = resolve(
        host_environment,
        "ATLASSIAN_HOST",
        crate::provider::Credential::AtlassianHost,
        "Atlassian host",
    )?;
    let (email, _) = resolve(
        email_environment,
        "ATLASSIAN_EMAIL",
        crate::provider::Credential::AtlassianEmail,
        "Atlassian email",
    )?;
    let (token, source) = resolve(
        token_environment,
        "ATLASSIAN_API_TOKEN",
        crate::provider::Credential::AtlassianToken,
        "Atlassian API token",
    )?;
    let host = crate::jira::normalize_host(&host)
        .map_err(|error| ResolveError::Failed(error.to_string()))?;
    Ok(ResolvedJira {
        credentials: JiraCredentials { host, email, token },
        source,
    })
}

fn jira_part(
    environment: Option<String>,
    credential: crate::provider::Credential,
    display_name: &str,
    store: &dyn SecretStore,
) -> Result<Option<(String, CredentialSource)>, ResolveError> {
    if let Some(value) = environment {
        return Ok(Some((value, CredentialSource::Environment)));
    }
    match store.get(credential) {
        Ok(Some(value)) => Ok(Some((value, CredentialSource::ConfigFile))),
        Ok(None) => Ok(None),
        Err(error) => Err(ResolveError::Failed(format!(
            "could not read {display_name} credential from the credentials file: {error}"
        ))),
    }
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
            Self::Jira => "jira",
            Self::Slack => "slack",
            Self::Sentry => "sentry",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Github => "GitHub",
            Self::Jira => "Jira",
            Self::Slack => "Slack",
            Self::Sentry => "Sentry",
        }
    }

    fn environment_variable(self) -> &'static str {
        match self {
            Self::Linear => "LINEAR_API_KEY",
            Self::Github => "GITHUB_TOKEN",
            Self::Jira => "ATLASSIAN_API_TOKEN",
            Self::Slack => "SLACK_BOT_TOKEN",
            Self::Sentry => "SENTRY_AUTH_TOKEN",
        }
    }

    fn credential(self) -> crate::provider::Credential {
        match self {
            Self::Linear => crate::provider::Credential::Linear,
            Self::Github => crate::provider::Credential::Github,
            Self::Jira => crate::provider::Credential::AtlassianToken,
            Self::Slack => crate::provider::Credential::SlackBot,
            Self::Sentry => crate::provider::Credential::Sentry,
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

impl SecretStore for crate::provider::CredentialStore {
    fn get(&self, credential: crate::provider::Credential) -> Result<Option<String>, String> {
        Ok(self
            .load()
            .map_err(|error| error.to_string())?
            .get(credential)
            .filter(|token| !token.is_empty())
            .map(str::to_owned))
    }

    fn set_many(&self, credentials: &[(crate::provider::Credential, &str)]) -> Result<(), String> {
        self.set_many(credentials)
            .map_err(|error| error.to_string())
    }

    fn delete_many(&self, credentials: &[crate::provider::Credential]) -> Result<bool, String> {
        self.delete_many(credentials)
            .map_err(|error| error.to_string())
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

fn run_slack(
    action: SlackAction,
    store: &dyn SecretStore,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SlackAction::Status => {
            print_status(Provider::Slack, store, format);
            Ok(())
        }
        SlackAction::Login => slack_login(store, format),
        SlackAction::Logout => logout(Provider::Slack, store, format),
    }
}

fn logout(
    provider: Provider,
    store: &dyn SecretStore,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let credentials: &[crate::provider::Credential] = match provider {
        Provider::Slack => &[
            crate::provider::Credential::SlackBot,
            crate::provider::Credential::SlackUser,
        ],
        Provider::Jira => &[
            crate::provider::Credential::AtlassianHost,
            crate::provider::Credential::AtlassianEmail,
            crate::provider::Credential::AtlassianToken,
        ],
        _ => &[provider.credential()],
    };
    let removed = store
        .delete_many(credentials)
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
    if provider == Provider::Sentry {
        store
            .get(provider.credential())
            .map_err(|error| format!("could not read Sentry credential: {error}"))?;
        if url.is_some() {
            crate::provider::SettingsStore.load()?;
        }
    }
    print_login_help(provider, url.as_deref())?;
    let token = read_token()?;
    if token.is_empty() {
        return Err("token cannot be empty".into());
    }
    let account = validate_and_store(provider, provider.credential(), &token, store, |token| {
        validate(provider, token, url.as_deref())
    })?;
    if let Some(url) = &url {
        crate::provider::SettingsStore.set_sentry_url(Some(url.clone()))?;
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

fn jira_login(
    host: Option<String>,
    email: Option<String>,
    store: &dyn SecretStore,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    print_login_help(Provider::Jira, None)?;
    let (host, email, token) = read_jira_login(host, email)?;
    let host = crate::jira::normalize_host(&host)?;
    let account = crate::jira::auth_identity(&host, &email, &token)
        .map(|identity| jira_account(&host, &identity))?;
    store
        .set_many(&[
            (crate::provider::Credential::AtlassianHost, host.as_str()),
            (crate::provider::Credential::AtlassianEmail, email.as_str()),
            (crate::provider::Credential::AtlassianToken, token.as_str()),
        ])
        .map_err(|error| format!("could not store Atlassian credentials: {error}"))?;
    for variable in ["ATLASSIAN_HOST", "ATLASSIAN_EMAIL", "ATLASSIAN_API_TOKEN"] {
        if environment(variable).is_some() {
            eprintln!(
                "Warning: {variable} is set and takes precedence over the stored credential."
            );
        }
    }
    let report = login_report(Provider::Jira, account);
    crate::output::print_text(
        &status_summary(Provider::Jira, &report[Provider::Jira.as_str()]),
        &report,
        format,
    );
    Ok(())
}

fn read_jira_login(
    host: Option<String>,
    email: Option<String>,
) -> Result<(String, String, String), Box<dyn std::error::Error>> {
    if std::io::stdin().is_terminal() {
        let host = match host {
            Some(host) => host,
            None => prompt_line("Atlassian host (e.g. acme.atlassian.net): ")?,
        };
        let email = match email {
            Some(email) => email,
            None => prompt_line("Email: ")?,
        };
        let token = rpassword::prompt_password("API token: ")?;
        return require_jira_values(host, email, token).map_err(Into::into);
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    parse_jira_login(&input, host, email).map_err(Into::into)
}

fn prompt_line(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    eprint!("{prompt}");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

fn parse_jira_login(
    input: &str,
    host: Option<String>,
    email: Option<String>,
) -> Result<(String, String, String), String> {
    const USAGE: &str =
        "Jira login input must contain one line per missing value, in host, email, token order";
    let mut lines = input.lines();
    let mut read = |value: Option<String>| match value {
        Some(value) => Ok(value),
        None => lines
            .next()
            .map(|line| line.trim().to_owned())
            .ok_or_else(|| USAGE.to_owned()),
    };
    let host = read(host)?;
    let email = read(email)?;
    let token = read(None)?;
    if lines.any(|line| !line.trim().is_empty()) {
        return Err(USAGE.to_owned());
    }
    require_jira_values(host, email, token)
}

fn require_jira_values(
    host: String,
    email: String,
    token: String,
) -> Result<(String, String, String), String> {
    for (value, what) in [(&host, "host"), (&email, "email"), (&token, "API token")] {
        if value.trim().is_empty() {
            return Err(format!("Atlassian {what} cannot be empty"));
        }
    }
    Ok((
        host.trim().to_owned(),
        email.trim().to_owned(),
        token.trim().to_owned(),
    ))
}

#[derive(Debug, PartialEq, Eq)]
struct SlackTokens {
    bot: Option<String>,
    user: Option<String>,
}

fn slack_login(
    store: &dyn SecretStore,
    format: crate::output::Format,
) -> Result<(), Box<dyn std::error::Error>> {
    print_login_help(Provider::Slack, None)?;
    let tokens = read_slack_tokens()?;
    let (bot_account, user_account) =
        validate_and_store_slack(&tokens, store, validate_slack_login_token)?;
    let mut stored = Vec::with_capacity(2);
    if tokens.bot.is_some() {
        stored.push("bot");
    }
    if tokens.user.is_some() {
        stored.push("user");
    }

    if tokens.bot.is_some() && environment_token(Provider::Slack).is_some() {
        eprintln!(
            "Warning: SLACK_BOT_TOKEN is set and takes precedence over the stored credential."
        );
    }
    if tokens.user.is_some() && environment_slack_user_token().is_some() {
        eprintln!(
            "Warning: SLACK_USER_TOKEN is set and takes precedence over the stored credential."
        );
    }

    let account = bot_account
        .or(user_account)
        .expect("at least one Slack token");
    let mut report = login_report(Provider::Slack, account);
    report[Provider::Slack.as_str()]["stored"] = json!(stored);
    crate::output::print_text(
        &status_summary(Provider::Slack, &report[Provider::Slack.as_str()]),
        &report,
        format,
    );
    Ok(())
}

fn validate_slack_login_token(token: &str, bot: bool) -> Result<Value, ValidationError> {
    let valid_form = if bot {
        crate::slack::is_bot_token(token)
    } else {
        crate::slack::is_user_token(token)
    };
    if !valid_form {
        let expected = if bot { "xoxb- bot" } else { "xoxp- user" };
        return Err(ValidationError::Rejected(format!(
            "Slack {expected} token required"
        )));
    }
    validate(Provider::Slack, token, None)
}

fn validate_and_store_slack<F>(
    tokens: &SlackTokens,
    store: &dyn SecretStore,
    mut validate: F,
) -> Result<(Option<Value>, Option<Value>), Box<dyn std::error::Error>>
where
    F: FnMut(&str, bool) -> Result<Value, ValidationError>,
{
    let bot_account = tokens
        .bot
        .as_deref()
        .map(|token| validate(token, true))
        .transpose()?;
    let user_account = tokens
        .user
        .as_deref()
        .map(|token| validate(token, false))
        .transpose()?;

    let mut credentials = Vec::with_capacity(2);
    if let Some(token) = tokens.bot.as_deref() {
        credentials.push((crate::provider::Credential::SlackBot, token));
    }
    if let Some(token) = tokens.user.as_deref() {
        credentials.push((crate::provider::Credential::SlackUser, token));
    }
    store
        .set_many(&credentials)
        .map_err(|error| format!("could not store Slack credentials: {error}"))?;
    Ok((bot_account, user_account))
}

fn read_slack_tokens() -> Result<SlackTokens, Box<dyn std::error::Error>> {
    if std::io::stdin().is_terminal() {
        return require_slack_token(SlackTokens {
            bot: nonempty_token(rpassword::prompt_password("Bot token (optional): ")?),
            user: nonempty_token(rpassword::prompt_password("User token (optional): ")?),
        });
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    parse_slack_tokens(&input).map_err(Into::into)
}

fn parse_slack_tokens(input: &str) -> Result<SlackTokens, String> {
    let mut lines = input.lines();
    let tokens = SlackTokens {
        bot: lines.next().map(str::to_owned).and_then(nonempty_token),
        user: lines.next().map(str::to_owned).and_then(nonempty_token),
    };
    if lines.any(|line| !line.trim().is_empty()) {
        return Err("Slack login input must contain two lines: bot token, then user token".into());
    }
    require_slack_token(tokens).map_err(|error| error.to_string())
}

fn require_slack_token(tokens: SlackTokens) -> Result<SlackTokens, Box<dyn std::error::Error>> {
    if tokens.bot.is_none() && tokens.user.is_none() {
        Err("at least one Slack token is required".into())
    } else {
        Ok(tokens)
    }
}

fn nonempty_token(token: String) -> Option<String> {
    let token = token.trim().to_owned();
    (!token.is_empty()).then_some(token)
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
    for provider in [
        Provider::Linear,
        Provider::Github,
        Provider::Jira,
        Provider::Slack,
        Provider::Sentry,
    ] {
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
        Provider::Jira => jira_identity(account),
        Provider::Slack => slack_identity(account),
        Provider::Sentry => sentry_identity(account),
    }
}

fn jira_identity(account: &Value) -> String {
    let name = text_field(account, "displayName");
    let email = text_field(account, "emailAddress");
    let host = text_field(account, "host");
    let person = match (name, email) {
        (Some(name), Some(email)) => format!("{name} <{email}>"),
        (Some(name), None) => name.to_owned(),
        (None, Some(email)) => email.to_owned(),
        (None, None) => String::new(),
    };
    match (person.is_empty(), host) {
        (false, Some(host)) => format!("{person}  {host}"),
        (false, None) => person,
        (true, Some(host)) => host.to_owned(),
        (true, None) => String::new(),
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

fn slack_identity(account: &Value) -> String {
    let user = text_field(account, "user");
    let team = text_field(account, "team");
    match (user, team) {
        (Some(user), Some(team)) => format!("{user}  {team}"),
        (Some(user), None) => user.to_owned(),
        (None, Some(team)) => team.to_owned(),
        (None, None) => String::new(),
    }
}

fn text_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

fn provider_status(provider: Provider, store: &dyn SecretStore) -> Value {
    // Jira validates with three credentials, not one token.
    if provider == Provider::Jira {
        return match resolve_jira(jira_environment(), store) {
            Ok(resolved) => credential_status(
                Ok(ResolvedCredential {
                    token: resolved.credentials.token.clone(),
                    source: resolved.source,
                }),
                |token| {
                    crate::jira::auth_identity(
                        &resolved.credentials.host,
                        &resolved.credentials.email,
                        token,
                    )
                    .map(|identity| jira_account(&resolved.credentials.host, &identity))
                },
            ),
            Err(error) => credential_status(Err(error), |_| unreachable!()),
        };
    }
    let resolved = match provider {
        Provider::Linear | Provider::Sentry => {
            resolve_stored(provider, environment_token(provider), store)
        }
        Provider::Github => resolve_github(environment_token(provider), store, github_cli_token),
        Provider::Slack => resolve_slack(
            environment_token(Provider::Slack),
            environment_slack_user_token(),
            store,
        ),
        Provider::Jira => unreachable!("handled above"),
    };
    credential_status(resolved, |token| validate(provider, token, None))
}

fn all_provider_statuses(store: &dyn SecretStore) -> Value {
    json!({
        "linear": provider_status(Provider::Linear, store),
        "github": provider_status(Provider::Github, store),
        "jira": provider_status(Provider::Jira, store),
        "slack": provider_status(Provider::Slack, store),
        "sentry": provider_status(Provider::Sentry, store),
    })
}

fn validate_and_store<F>(
    provider: Provider,
    credential: crate::provider::Credential,
    token: &str,
    store: &dyn SecretStore,
    validate: F,
) -> Result<Value, Box<dyn std::error::Error>>
where
    F: FnOnce(&str) -> Result<Value, ValidationError>,
{
    let account = validate(token)?;
    store
        .set_many(&[(credential, token)])
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
    match store.get(provider.credential()) {
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
            "could not read {} credential from the credentials file: {error}",
            provider.display_name(),
        ))),
    }
}

fn resolve_slack(
    bot_environment: Option<String>,
    user_environment: Option<String>,
    store: &dyn SecretStore,
) -> Result<ResolvedCredential, ResolveError> {
    match resolve_slack_bot(bot_environment, store) {
        Ok(credential) => Ok(credential),
        Err(ResolveError::Missing(_)) => match resolve_slack_user(user_environment, store) {
            Ok(credential) => Ok(credential),
            Err(ResolveError::Missing(_)) => Err(ResolveError::Missing(
                "SLACK_BOT_TOKEN and SLACK_USER_TOKEN are not set and no Slack credential is stored"
                    .into(),
            )),
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    }
}

fn resolve_slack_bot(
    environment: Option<String>,
    store: &dyn SecretStore,
) -> Result<ResolvedCredential, ResolveError> {
    let credential = resolve_named_stored(
        environment,
        &[crate::provider::Credential::SlackBot],
        "SLACK_BOT_TOKEN is not set and no Slack bot credential is stored",
        "Slack bot",
        store,
    )?;
    if !crate::slack::is_bot_token(&credential.token) {
        return Err(ResolveError::Failed(
            "SLACK_BOT_TOKEN and stored Slack credentials must be xoxb- bot tokens".into(),
        ));
    }
    Ok(credential)
}

fn resolve_slack_user(
    environment: Option<String>,
    store: &dyn SecretStore,
) -> Result<ResolvedCredential, ResolveError> {
    let credential = resolve_named_stored(
        environment,
        &[crate::provider::Credential::SlackUser],
        "SLACK_USER_TOKEN is not set and no Slack user credential is stored; Slack search requires a user token with search:read",
        "Slack user",
        store,
    )?;
    if !crate::slack::is_user_token(&credential.token) {
        return Err(ResolveError::Failed(
            "SLACK_USER_TOKEN and stored Slack user credentials must be xoxp- user tokens".into(),
        ));
    }
    Ok(credential)
}

fn resolve_named_stored(
    environment: Option<String>,
    stored_credentials: &[crate::provider::Credential],
    missing: &str,
    display_name: &str,
    store: &dyn SecretStore,
) -> Result<ResolvedCredential, ResolveError> {
    if let Some(token) = environment {
        return Ok(ResolvedCredential {
            token,
            source: CredentialSource::Environment,
        });
    }
    for credential in stored_credentials {
        match store.get(*credential) {
            Ok(Some(token)) => {
                return Ok(ResolvedCredential {
                    token,
                    source: CredentialSource::ConfigFile,
                });
            }
            Ok(None) => {}
            Err(error) => {
                return Err(ResolveError::Failed(format!(
                    "could not read {display_name} credential from the credentials file: {error}"
                )));
            }
        }
    }
    Err(ResolveError::Missing(missing.into()))
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
    let store_error = match store.get(Provider::Github.credential()) {
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
                "could not read GitHub credential from the credentials file: {error}"
            ))),
            None => Err(ResolveError::Missing(
                "GITHUB_TOKEN is not set, no GitHub credential is stored, and `gh auth token` did not return a token".into(),
            )),
        },
        Err(error) => Err(ResolveError::Failed(match store_error {
            Some(store_error) => format!(
                "could not read GitHub credential from the credentials file ({store_error}) or GitHub CLI ({error})"
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
        Provider::Jira => unreachable!("Jira validates with host and email, not one token"),
        Provider::Slack => crate::slack::auth_identity(token).map(slack_account),
        Provider::Sentry => crate::sentry::auth_identity(token, sentry_url).map(sentry_account),
    }
}

fn jira_account(host: &str, identity: &Value) -> Value {
    json!({
        "accountId": identity["accountId"],
        "displayName": identity["displayName"],
        "emailAddress": identity["emailAddress"],
        "host": host,
    })
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

fn slack_account(identity: Value) -> Value {
    json!({
        "team_id": identity["team_id"],
        "team": identity["team"],
        "user_id": identity["user_id"],
        "user": identity["user"],
        "bot_id": identity["bot_id"],
        "token_type": if identity["bot_id"].as_str().is_some_and(|id| !id.is_empty()) {
            "bot"
        } else {
            "user"
        },
    })
}

fn environment(variable: &str) -> Option<String> {
    std::env::var(variable)
        .ok()
        .filter(|value| !value.is_empty())
}

fn environment_token(provider: Provider) -> Option<String> {
    environment(provider.environment_variable())
}

fn environment_slack_user_token() -> Option<String> {
    environment("SLACK_USER_TOKEN")
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
/// to `SENTRY_URL`, then the settings file, then the default.
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

fn print_login_help(
    provider: Provider,
    sentry_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match provider {
        Provider::Linear => eprintln!(
            "Create a personal API key at https://linear.app/settings/account/security and grant the permissions needed by your foac commands."
        ),
        Provider::Github => eprintln!(
            "Create a fine-grained personal access token at https://github.com/settings/personal-access-tokens/new and grant the repository permissions needed by your foac commands."
        ),
        Provider::Jira => eprintln!(
            "Create an Atlassian API token at https://id.atlassian.com/manage-profile/security/api-tokens (the token covers Jira and Confluence), and have your site host (like acme.atlassian.net) and account email ready."
        ),
        Provider::Slack => eprintln!(
            "Go to https://api.slack.com/apps and choose Create New App > From a manifest.\nSuggested manifest:\n{SLACK_APP_MANIFEST}\nInstall the app to your workspace from OAuth & Permissions, then enter its Bot User OAuth Token (xoxb-) and User OAuth Token (xoxp-). Leave either prompt blank if that token type is not needed."
        ),
        Provider::Sentry => {
            let url = match sentry_url {
                Some(url) => url.to_owned(),
                None => crate::sentry::base_url()?,
            };
            eprintln!(
                "Create a user auth token at {url}/settings/account/api/auth-tokens/ and grant the scopes needed by your foac commands."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    const LINEAR_CREDENTIAL: crate::provider::Credential = crate::provider::Credential::Linear;
    const GITHUB_CREDENTIAL: crate::provider::Credential = crate::provider::Credential::Github;
    const SLACK_BOT_CREDENTIAL: crate::provider::Credential = crate::provider::Credential::SlackBot;
    const SLACK_USER_CREDENTIAL: crate::provider::Credential =
        crate::provider::Credential::SlackUser;

    #[derive(Default)]
    struct MemoryStore {
        credentials: RefCell<HashMap<crate::provider::Credential, String>>,
        get_error: Option<String>,
    }

    impl SecretStore for MemoryStore {
        fn get(&self, credential: crate::provider::Credential) -> Result<Option<String>, String> {
            if let Some(error) = &self.get_error {
                return Err(error.clone());
            }
            Ok(self.credentials.borrow().get(&credential).cloned())
        }

        fn set_many(
            &self,
            credentials: &[(crate::provider::Credential, &str)],
        ) -> Result<(), String> {
            let mut stored = self.credentials.borrow_mut();
            for (credential, token) in credentials {
                stored.insert(*credential, (*token).to_owned());
            }
            Ok(())
        }

        fn delete_many(&self, credentials: &[crate::provider::Credential]) -> Result<bool, String> {
            let mut stored = self.credentials.borrow_mut();
            let mut removed = false;
            for credential in credentials {
                removed = stored.remove(credential).is_some() || removed;
            }
            Ok(removed)
        }
    }

    #[test]
    fn linear_credentials_prefer_environment_then_secret_store() {
        let store = MemoryStore::default();
        store.set_many(&[(LINEAR_CREDENTIAL, "stored")]).unwrap();
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
    fn jira_credentials_resolve_environment_then_store_and_name_the_missing_part() {
        let store = MemoryStore::default();
        let missing = resolve_jira([None, None, None], &store).unwrap_err();
        assert!(matches!(missing, ResolveError::Missing(_)));
        assert!(missing.to_string().contains("ATLASSIAN_HOST"));

        store
            .set_many(&[
                (
                    crate::provider::Credential::AtlassianHost,
                    "https://acme.atlassian.net/",
                ),
                (
                    crate::provider::Credential::AtlassianEmail,
                    "user@example.com",
                ),
            ])
            .unwrap();
        let missing = resolve_jira([None, None, None], &store).unwrap_err();
        assert!(missing.to_string().contains("ATLASSIAN_API_TOKEN"));

        store
            .set_many(&[(crate::provider::Credential::AtlassianToken, "stored-token")])
            .unwrap();
        let resolved = resolve_jira([None, None, None], &store).unwrap();
        assert_eq!(resolved.credentials.host, "acme.atlassian.net");
        assert_eq!(resolved.credentials.email, "user@example.com");
        assert_eq!(resolved.credentials.token, "stored-token");
        assert_eq!(resolved.source, CredentialSource::ConfigFile);

        // The environment beats the store part by part; the token decides the
        // reported source.
        let resolved = resolve_jira(
            [
                Some("env.atlassian.net".into()),
                None,
                Some("env-token".into()),
            ],
            &store,
        )
        .unwrap();
        assert_eq!(resolved.credentials.host, "env.atlassian.net");
        assert_eq!(resolved.credentials.email, "user@example.com");
        assert_eq!(resolved.credentials.token, "env-token");
        assert_eq!(resolved.source, CredentialSource::Environment);

        let broken = MemoryStore {
            get_error: Some("locked".into()),
            ..Default::default()
        };
        assert!(matches!(
            resolve_jira([None, None, None], &broken),
            Err(ResolveError::Failed(_))
        ));
    }

    #[test]
    fn parses_jira_login_lines_for_the_values_flags_do_not_cover() {
        assert_eq!(
            parse_jira_login("acme.atlassian.net\nuser@example.com\ntoken\n", None, None).unwrap(),
            (
                "acme.atlassian.net".into(),
                "user@example.com".into(),
                "token".into()
            )
        );
        assert_eq!(
            parse_jira_login(
                "token\n",
                Some("acme.atlassian.net".into()),
                Some("user@example.com".into())
            )
            .unwrap(),
            (
                "acme.atlassian.net".into(),
                "user@example.com".into(),
                "token".into()
            )
        );
        assert!(parse_jira_login("only-a-token\n", None, None).is_err());
        assert!(parse_jira_login("host\nemail\ntoken\nextra\n", None, None).is_err());
        assert!(parse_jira_login("host\n\ntoken\n", None, None).is_err());
    }

    #[test]
    fn jira_account_and_identity_keep_safe_fields() {
        let account = jira_account(
            "acme.atlassian.net",
            &json!({
                "accountId": "5b10a2844c20165700ede21g",
                "displayName": "User",
                "emailAddress": "user@example.com",
                "active": true,
            }),
        );
        assert_eq!(
            account,
            json!({
                "accountId": "5b10a2844c20165700ede21g",
                "displayName": "User",
                "emailAddress": "user@example.com",
                "host": "acme.atlassian.net",
            })
        );
        assert_eq!(
            account_identity(Provider::Jira, &account),
            "User <user@example.com>  acme.atlassian.net"
        );
    }

    #[test]
    fn slack_credentials_follow_the_capability_matrix() {
        let store = MemoryStore::default();

        let missing = resolve_slack(None, None, &store).unwrap_err();
        assert!(matches!(missing, ResolveError::Missing(_)));
        assert!(missing.to_string().contains("SLACK_BOT_TOKEN"));
        assert!(missing.to_string().contains("SLACK_USER_TOKEN"));

        let bot = resolve_slack(Some("xoxb-environment".into()), None, &store).unwrap();
        assert_eq!(bot.token, "xoxb-environment");
        assert_eq!(bot.source, CredentialSource::Environment);

        let user = resolve_slack(None, Some("xoxp-user".into()), &store).unwrap();
        assert_eq!(user.token, "xoxp-user");
        assert_eq!(user.source, CredentialSource::Environment);

        let bot = resolve_slack(
            Some("xoxb-environment".into()),
            Some("xoxp-user".into()),
            &store,
        )
        .unwrap();
        assert_eq!(bot.token, "xoxb-environment");

        let stored_user = MemoryStore::default();
        stored_user
            .set_many(&[(SLACK_USER_CREDENTIAL, "xoxp-stored-user")])
            .unwrap();
        let user = resolve_slack(None, None, &stored_user).unwrap();
        assert_eq!(user.token, "xoxp-stored-user");
        assert_eq!(user.source, CredentialSource::ConfigFile);
        assert_eq!(
            resolve_slack_user(None, &stored_user).unwrap().token,
            "xoxp-stored-user"
        );
    }

    #[test]
    fn slack_stored_bot_precedes_user_and_invalid_tokens_fail() {
        let store = MemoryStore::default();
        store
            .set_many(&[(SLACK_BOT_CREDENTIAL, "xoxb-stored")])
            .unwrap();
        let resolved = resolve_slack(None, Some("xoxp-user".into()), &store).unwrap();
        assert_eq!(resolved.token, "xoxb-stored");
        assert_eq!(resolved.source, CredentialSource::ConfigFile);

        let error = resolve_slack(
            Some("not-a-bot-token".into()),
            Some("xoxp-user".into()),
            &store,
        )
        .unwrap_err();
        assert!(matches!(error, ResolveError::Failed(_)));
        assert!(error.to_string().contains("xoxb-"));

        let error = resolve_slack_user(Some("not-a-user-token".into()), &store).unwrap_err();
        assert!(matches!(error, ResolveError::Failed(_)));
        assert!(error.to_string().contains("xoxp-"));
    }

    #[test]
    fn slack_stored_tokens_have_independent_environment_precedence() {
        let store = MemoryStore::default();
        store
            .set_many(&[
                (SLACK_BOT_CREDENTIAL, "xoxb-stored"),
                (SLACK_USER_CREDENTIAL, "xoxp-stored"),
            ])
            .unwrap();

        assert_eq!(
            resolve_slack(None, None, &store).unwrap().token,
            "xoxb-stored"
        );
        assert_eq!(
            resolve_slack_user(None, &store).unwrap().token,
            "xoxp-stored"
        );
        assert_eq!(
            resolve_slack(Some("xoxb-env".into()), None, &store)
                .unwrap()
                .token,
            "xoxb-env"
        );
        assert_eq!(
            resolve_slack_user(Some("xoxp-env".into()), &store)
                .unwrap()
                .token,
            "xoxp-env"
        );
    }

    #[test]
    fn parses_slack_login_tokens_in_bot_then_user_order() {
        assert_eq!(
            parse_slack_tokens("xoxb-bot\nxoxp-user\n").unwrap(),
            SlackTokens {
                bot: Some("xoxb-bot".into()),
                user: Some("xoxp-user".into()),
            }
        );
        assert_eq!(
            parse_slack_tokens("\nxoxp-user\n").unwrap(),
            SlackTokens {
                bot: None,
                user: Some("xoxp-user".into()),
            }
        );
        assert!(parse_slack_tokens("\n\n").is_err());
        assert!(parse_slack_tokens("bot\nuser\nextra").is_err());
    }

    #[test]
    fn slack_login_validates_both_tokens_before_storing_either() {
        let store = MemoryStore::default();
        store
            .set_many(&[
                (SLACK_BOT_CREDENTIAL, "xoxb-existing"),
                (SLACK_USER_CREDENTIAL, "xoxp-existing"),
            ])
            .unwrap();
        let tokens = SlackTokens {
            bot: Some("xoxb-new".into()),
            user: Some("xoxp-bad".into()),
        };
        let result = validate_and_store_slack(&tokens, &store, |token, _| {
            if token == "xoxp-bad" {
                Err(ValidationError::Rejected("rejected".into()))
            } else {
                Ok(json!({"user": "foac"}))
            }
        });
        assert!(result.is_err());
        assert_eq!(
            store.get(SLACK_BOT_CREDENTIAL).unwrap().as_deref(),
            Some("xoxb-existing")
        );
        assert_eq!(
            store.get(SLACK_USER_CREDENTIAL).unwrap().as_deref(),
            Some("xoxp-existing")
        );

        let result = validate_and_store_slack(&tokens, &store, |token, bot| {
            Ok(json!({
                "user": token,
                "token_type": if bot { "bot" } else { "user" },
            }))
        })
        .unwrap();
        assert_eq!(result.0.unwrap()["token_type"], "bot");
        assert_eq!(result.1.unwrap()["token_type"], "user");
        assert_eq!(
            store.get(SLACK_BOT_CREDENTIAL).unwrap().as_deref(),
            Some("xoxb-new")
        );
        assert_eq!(
            store.get(SLACK_USER_CREDENTIAL).unwrap().as_deref(),
            Some("xoxp-bad")
        );
    }

    #[test]
    fn slack_login_partial_updates_preserve_the_other_token() {
        let store = MemoryStore::default();
        store
            .set_many(&[
                (SLACK_BOT_CREDENTIAL, "xoxb-existing"),
                (SLACK_USER_CREDENTIAL, "xoxp-existing"),
            ])
            .unwrap();
        let validate = |token: &str, _| Ok(json!({"user": token}));

        validate_and_store_slack(
            &SlackTokens {
                bot: Some("xoxb-new".into()),
                user: None,
            },
            &store,
            validate,
        )
        .unwrap();
        assert_eq!(
            store.get(SLACK_BOT_CREDENTIAL).unwrap().as_deref(),
            Some("xoxb-new")
        );
        assert_eq!(
            store.get(SLACK_USER_CREDENTIAL).unwrap().as_deref(),
            Some("xoxp-existing")
        );

        validate_and_store_slack(
            &SlackTokens {
                bot: None,
                user: Some("xoxp-new".into()),
            },
            &store,
            validate,
        )
        .unwrap();
        assert_eq!(
            store.get(SLACK_BOT_CREDENTIAL).unwrap().as_deref(),
            Some("xoxb-new")
        );
        assert_eq!(
            store.get(SLACK_USER_CREDENTIAL).unwrap().as_deref(),
            Some("xoxp-new")
        );
    }

    #[test]
    fn slack_logout_removes_both_tokens() {
        let store = MemoryStore::default();
        store
            .set_many(&[
                (SLACK_BOT_CREDENTIAL, "xoxb-bot"),
                (SLACK_USER_CREDENTIAL, "xoxp-user"),
            ])
            .unwrap();
        logout(Provider::Slack, &store, crate::output::Format::Json).unwrap();
        assert!(store.credentials.borrow().is_empty());
    }

    #[test]
    fn memory_store_logout_is_idempotent() {
        let store = MemoryStore::default();
        store.set_many(&[(GITHUB_CREDENTIAL, "token")]).unwrap();
        assert!(store.delete_many(&[GITHUB_CREDENTIAL]).unwrap());
        assert!(!store.delete_many(&[GITHUB_CREDENTIAL]).unwrap());
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

        let slack = slack_account(json!({
            "ok": true,
            "team_id": "T1",
            "team": "Acme",
            "user_id": "U1",
            "user": "foac",
            "bot_id": "B1",
            "url": "https://acme.slack.com/",
        }));
        assert_eq!(
            slack,
            json!({
                "team_id": "T1",
                "team": "Acme",
                "user_id": "U1",
                "user": "foac",
                "bot_id": "B1",
                "token_type": "bot",
            })
        );

        let slack_user = slack_account(json!({
            "ok": true,
            "team_id": "T1",
            "team": "Acme",
            "user_id": "U2",
            "user": "person",
        }));
        assert_eq!(slack_user["token_type"], "user");
        assert!(slack_user["bot_id"].is_null());
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
                "name": "User",
                "displayName": "Display",
                "email": "user@example.com",
            },
            "organization": { "id": "ws", "name": "Workspace", "urlKey": "workspace" },
        }));
        assert_eq!(
            account_identity(Provider::Linear, &linear),
            "User <user@example.com>  Workspace"
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

        let slack = slack_account(json!({
            "team_id": "T1",
            "team": "Acme",
            "user_id": "U1",
            "user": "foac",
            "bot_id": "B1",
        }));
        assert_eq!(account_identity(Provider::Slack, &slack), "foac  Acme");

        let sentry = sentry_account(json!([
            { "id": "1", "slug": "acme", "name": "Acme" },
            { "id": "2", "slug": "globex", "name": "Globex" },
        ]));
        assert_eq!(account_identity(Provider::Sentry, &sentry), "acme, globex");
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
                        "name": "User",
                        "email": "user@example.com",
                        "workspace": { "name": "Workspace" },
                    },
                })
            ),
            "authenticated via environment\nUser <user@example.com>  Workspace\n"
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
                    "name": "User",
                    "email": "user@example.com",
                    "workspace": { "name": "Workspace" },
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
            "User <user@example.com>  Workspace"
        );
        assert!(table["github"].get("account").is_none());
        assert_eq!(statuses["linear"]["account"]["email"], "user@example.com");
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
        store
            .set_many(&[(LINEAR_CREDENTIAL, "existing-token")])
            .unwrap();

        let result = validate_and_store(
            Provider::Linear,
            LINEAR_CREDENTIAL,
            "bad-token",
            &store,
            |_| Err(ValidationError::Rejected("rejected".into())),
        );
        assert!(result.is_err());
        assert_eq!(
            store.get(LINEAR_CREDENTIAL).unwrap().as_deref(),
            Some("existing-token")
        );

        let account = json!({ "id": "user-id" });
        let result = validate_and_store(
            Provider::Linear,
            LINEAR_CREDENTIAL,
            "new-token",
            &store,
            |_| Ok(account.clone()),
        )
        .unwrap();
        assert_eq!(result, account);
        assert_eq!(
            store.get(LINEAR_CREDENTIAL).unwrap().as_deref(),
            Some("new-token")
        );
    }
}
