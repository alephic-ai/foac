//! Atlassian vendor code shared by the Jira and Confluence providers: the
//! host/email/API-token credential triple, its resolution and login flow,
//! and the Basic-auth `Api` both providers request through. Each provider
//! keeps its own command tree; this file is the vendor, not a provider.

use std::io::{IsTerminal, Read};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::auth::{
    CredentialSource, Provider, ResolveError, SecretStore, ValidationError, environment,
    text_field, vendor_has_stored_instances,
};
use crate::provider::DEFAULT_INSTANCE;
use crate::rest::{self, Api, Auth};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub(crate) struct AtlassianCmd {
    #[command(subcommand)]
    pub(crate) command: AtlassianAction,
}

#[derive(Subcommand)]
pub(crate) enum AtlassianAction {
    /// Check authentication for this provider
    Status,
    /// Validate and save the Atlassian host, email, and API token
    ///
    /// Prompts for the host, email, then API token. Redirected input supplies
    /// one line per missing value in the same order; --host and --email skip
    /// their prompt or line. The stored credential is shared between Jira and
    /// Confluence: logging in through either covers both, and logging out
    /// removes it for both.
    Login {
        /// Atlassian site host like acme.atlassian.net, skipping the prompt
        #[arg(long)]
        host: Option<String>,
        /// Atlassian account email, skipping the prompt
        #[arg(long)]
        email: Option<String>,
    },
    /// Remove foac's stored Atlassian host, email, and API token,
    /// de-authenticating both Jira and Confluence
    Logout,
}

/// Vendor-level Atlassian credentials: every Jira and Confluence request
/// needs the tenant host, the account email, and the API token.
#[derive(Debug)]
pub(crate) struct AtlassianCredentials {
    pub(crate) host: String,
    pub(crate) email: String,
    pub(crate) token: String,
}

#[derive(Debug)]
pub(crate) struct ResolvedAtlassian {
    pub(crate) credentials: AtlassianCredentials,
    /// The token's source; the host and email may come from elsewhere.
    pub(crate) source: CredentialSource,
}

/// The Basic-auth `Api` both providers use against the tenant host.
pub(crate) fn api(
    credentials: AtlassianCredentials,
    format: crate::output::Format,
) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(&format!("https://{}", credentials.host))?,
        auth: Auth::Basic {
            user: credentials.email,
            password: credentials.token,
        },
        format,
        headers: Vec::new(),
        trailing_slash: false,
    })
}

/// Fetch the identity resource at `url` with Basic auth; each provider
/// supplies its own path, since Jira and Confluence expose it differently.
pub(crate) fn identity(url: &str, email: &str, token: &str) -> Result<Value, ValidationError> {
    let url =
        reqwest::Url::parse(url).map_err(|error| ValidationError::Failed(error.to_string()))?;
    rest::identity(
        url,
        &Auth::Basic {
            user: email.to_owned(),
            password: token.to_owned(),
        },
        &[],
        &[
            reqwest::StatusCode::UNAUTHORIZED,
            reqwest::StatusCode::FORBIDDEN,
        ],
    )
}

/// Turn user input into a bare host: whitespace, a pasted scheme, and
/// trailing slashes are dropped.
pub(crate) fn normalize_host(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    let host = input.trim();
    let host = host.split_once("://").map_or(host, |(_, host)| host);
    let host = host.trim_end_matches('/');
    if host.is_empty() {
        return Err("Atlassian host cannot be empty".into());
    }
    Ok(host.to_owned())
}

/// Whether Jira and Confluence are usable: the default instance resolves, or
/// any named instance is stored.
pub fn authenticated() -> bool {
    resolve_atlassian(
        atlassian_environment(),
        &crate::provider::CredentialStore,
        DEFAULT_INSTANCE,
    )
    .is_ok()
        || vendor_has_stored_instances("atlassian")
}

/// Resolve the credentials for a `foac jira` or `foac confluence`
/// invocation; `login` names the calling provider's login command for error
/// messages. Each value falls back flag > environment > stored; a missing
/// token is additionally read from redirected stdin so a one-off invocation
/// never puts it in shell history.
pub(crate) fn atlassian_credentials(
    host_flag: Option<String>,
    email_flag: Option<String>,
    login: &str,
    instance: &str,
) -> Result<AtlassianCredentials, Box<dyn std::error::Error>> {
    let store = crate::provider::CredentialStore;
    let login = login_command(login, instance);
    let host = match host_flag {
        Some(host) => Some(host),
        None => atlassian_part(
            environment("ATLASSIAN_HOST"),
            crate::provider::Credential::AtlassianHost,
            "Atlassian host",
            &store,
            instance,
        )?
        .map(|(value, _)| value),
    }
    .ok_or_else(|| {
        missing_atlassian_part(
            "--host or ATLASSIAN_HOST",
            "Atlassian host",
            &login,
            instance,
        )
    })?;
    let email = match email_flag {
        Some(email) => Some(email),
        None => atlassian_part(
            environment("ATLASSIAN_EMAIL"),
            crate::provider::Credential::AtlassianEmail,
            "Atlassian email",
            &store,
            instance,
        )?
        .map(|(value, _)| value),
    }
    .ok_or_else(|| {
        missing_atlassian_part(
            "--email or ATLASSIAN_EMAIL",
            "Atlassian email",
            &login,
            instance,
        )
    })?;
    let token = match atlassian_part(
        environment("ATLASSIAN_API_TOKEN"),
        crate::provider::Credential::AtlassianToken,
        "Atlassian API token",
        &store,
        instance,
    )? {
        Some((token, _)) => token,
        None if !std::io::stdin().is_terminal() => {
            let token = crate::auth::read_token()?;
            if token.is_empty() {
                return Err("the Atlassian API token piped to stdin is empty".into());
            }
            token
        }
        None if instance == DEFAULT_INSTANCE => {
            return Err(format!(
                "ATLASSIAN_API_TOKEN is not set and no Atlassian API token is stored; pipe the token to stdin or run `{login}`"
            )
            .into());
        }
        None => {
            return Err(format!(
                "no Atlassian API token is stored for instance \"{instance}\"; pipe the token to stdin or run `{login}`"
            )
            .into());
        }
    };
    Ok(AtlassianCredentials {
        host: normalize_host(&host)?,
        email,
        token,
    })
}

/// A provider's login command, with `--instance` appended for named instances.
fn login_command(login: &str, instance: &str) -> String {
    if instance == DEFAULT_INSTANCE {
        login.to_owned()
    } else {
        format!("{login} --instance {instance}")
    }
}

fn missing_atlassian_part(sources: &str, what: &str, login: &str, instance: &str) -> String {
    if instance == DEFAULT_INSTANCE {
        format!("{sources} is not set and no {what} is stored; run `{login}`")
    } else {
        format!("no {what} is stored for instance \"{instance}\"; run `{login}`")
    }
}

/// The `ATLASSIAN_HOST`, `ATLASSIAN_EMAIL`, and `ATLASSIAN_API_TOKEN`
/// environment values, in that order.
pub(crate) fn atlassian_environment() -> [Option<String>; 3] {
    [
        environment("ATLASSIAN_HOST"),
        environment("ATLASSIAN_EMAIL"),
        environment("ATLASSIAN_API_TOKEN"),
    ]
}

pub(crate) fn resolve_atlassian(
    environment: [Option<String>; 3],
    store: &dyn SecretStore,
    instance: &str,
) -> Result<ResolvedAtlassian, ResolveError> {
    let [host_environment, email_environment, token_environment] = environment;
    let resolve = |environment,
                   variable: &str,
                   credential,
                   display_name: &str|
     -> Result<(String, CredentialSource), ResolveError> {
        atlassian_part(environment, credential, display_name, store, instance)?.ok_or_else(|| {
            ResolveError::Missing(if instance == DEFAULT_INSTANCE {
                format!("{variable} is not set and no {display_name} is stored")
            } else {
                format!("no {display_name} is stored for instance \"{instance}\"")
            })
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
    let host = normalize_host(&host).map_err(|error| ResolveError::Failed(error.to_string()))?;
    Ok(ResolvedAtlassian {
        credentials: AtlassianCredentials { host, email, token },
        source,
    })
}

fn atlassian_part(
    environment: Option<String>,
    credential: crate::provider::Credential,
    display_name: &str,
    store: &dyn SecretStore,
    instance: &str,
) -> Result<Option<(String, CredentialSource)>, ResolveError> {
    // Environment values belong to the default instance only.
    if instance == DEFAULT_INSTANCE
        && let Some(value) = environment
    {
        return Ok(Some((value, CredentialSource::Environment)));
    }
    match store.get(credential, instance) {
        Ok(Some(value)) => Ok(Some((value, CredentialSource::ConfigFile))),
        Ok(None) => Ok(None),
        Err(error) => Err(ResolveError::Failed(format!(
            "could not read {display_name} credential from the credentials file: {error}"
        ))),
    }
}

pub(crate) fn run_atlassian(
    provider: Provider,
    action: AtlassianAction,
    store: &dyn SecretStore,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        AtlassianAction::Status => {
            crate::auth::print_status(provider, store, format, instance);
            Ok(())
        }
        AtlassianAction::Login { host, email } => {
            atlassian_login(provider, host, email, store, format, instance)
        }
        AtlassianAction::Logout => crate::auth::logout(provider, store, format, instance),
    }
}

fn atlassian_login(
    provider: Provider,
    host: Option<String>,
    email: Option<String>,
    store: &dyn SecretStore,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::auth::print_login_help(provider, None, instance)?;
    let (host, email, token) = read_atlassian_login(host, email)?;
    let host = normalize_host(&host)?;
    let account = atlassian_account(provider, &host, &email, &token)?;
    store
        .set_many(
            &[
                (crate::provider::Credential::AtlassianHost, host.as_str()),
                (crate::provider::Credential::AtlassianEmail, email.as_str()),
                (crate::provider::Credential::AtlassianToken, token.as_str()),
            ],
            instance,
        )
        .map_err(|error| format!("could not store Atlassian credentials: {error}"))?;
    if instance == DEFAULT_INSTANCE {
        for variable in ["ATLASSIAN_HOST", "ATLASSIAN_EMAIL", "ATLASSIAN_API_TOKEN"] {
            if environment(variable).is_some() {
                eprintln!(
                    "Warning: {variable} is set and takes precedence over the stored credential."
                );
            }
        }
    }
    let report = crate::auth::login_report(provider, instance, account);
    crate::output::print_text(
        &crate::auth::status_summary(
            provider,
            &report[crate::auth::status_key(provider, instance)],
        ),
        &report,
        format,
    );
    Ok(())
}

/// Validate the shared Atlassian credential against the calling provider's
/// API, so a Confluence login fails on a Jira-only tenant and vice versa.
pub(crate) fn atlassian_account(
    provider: Provider,
    host: &str,
    email: &str,
    token: &str,
) -> Result<Value, ValidationError> {
    match provider {
        Provider::Jira => crate::jira::auth_identity(host, email, token)
            .map(|identity| jira_account(host, &identity)),
        Provider::Confluence => crate::confluence::auth_identity(host, email, token)
            .map(|identity| confluence_account(host, &identity)),
        _ => unreachable!("not an Atlassian provider"),
    }
}

fn read_atlassian_login(
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
        return require_atlassian_values(host, email, token).map_err(Into::into);
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    parse_atlassian_login(&input, host, email).map_err(Into::into)
}

fn prompt_line(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    eprint!("{prompt}");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

fn parse_atlassian_login(
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
    require_atlassian_values(host, email, token)
}

fn require_atlassian_values(
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

/// The auth-table summary for either Atlassian provider; both account
/// shapes carry `emailAddress`.
pub(crate) fn jira_identity(account: &Value) -> String {
    crate::auth::person_identity(
        text_field(account, "displayName"),
        text_field(account, "emailAddress"),
        text_field(account, "host"),
    )
}

fn jira_account(host: &str, identity: &Value) -> Value {
    json!({
        "accountId": identity["accountId"],
        "displayName": identity["displayName"],
        "emailAddress": identity["emailAddress"],
        "host": host,
    })
}

/// Confluence's user resource calls the email field `email` where Jira says
/// `emailAddress`; map both to the same account shape.
fn confluence_account(host: &str, identity: &Value) -> Value {
    json!({
        "accountId": identity["accountId"],
        "displayName": identity["displayName"],
        "emailAddress": identity["email"],
        "host": host,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::tests::MemoryStore;

    #[test]
    fn normalizes_hosts_and_rejects_empty_input() {
        assert_eq!(
            normalize_host(" acme.atlassian.net \n").unwrap(),
            "acme.atlassian.net"
        );
        assert_eq!(
            normalize_host("https://acme.atlassian.net/").unwrap(),
            "acme.atlassian.net"
        );
        assert!(normalize_host(" \n").is_err());
        assert!(normalize_host("https:///").is_err());
    }

    #[test]
    fn atlassian_credentials_resolve_environment_then_store_and_name_the_missing_part() {
        let store = MemoryStore::default();
        let missing = resolve_atlassian([None, None, None], &store, DEFAULT_INSTANCE).unwrap_err();
        assert!(matches!(missing, ResolveError::Missing(_)));
        assert!(missing.to_string().contains("ATLASSIAN_HOST"));

        store
            .set_many(
                &[
                    (
                        crate::provider::Credential::AtlassianHost,
                        "https://acme.atlassian.net/",
                    ),
                    (
                        crate::provider::Credential::AtlassianEmail,
                        "user@example.com",
                    ),
                ],
                DEFAULT_INSTANCE,
            )
            .unwrap();
        let missing = resolve_atlassian([None, None, None], &store, DEFAULT_INSTANCE).unwrap_err();
        assert!(missing.to_string().contains("ATLASSIAN_API_TOKEN"));

        store
            .set_many(
                &[(crate::provider::Credential::AtlassianToken, "stored-token")],
                DEFAULT_INSTANCE,
            )
            .unwrap();
        let resolved = resolve_atlassian([None, None, None], &store, DEFAULT_INSTANCE).unwrap();
        assert_eq!(resolved.credentials.host, "acme.atlassian.net");
        assert_eq!(resolved.credentials.email, "user@example.com");
        assert_eq!(resolved.credentials.token, "stored-token");
        assert_eq!(resolved.source, CredentialSource::ConfigFile);

        // The environment beats the store part by part; the token decides the
        // reported source.
        let resolved = resolve_atlassian(
            [
                Some("env.atlassian.net".into()),
                None,
                Some("env-token".into()),
            ],
            &store,
            DEFAULT_INSTANCE,
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
            resolve_atlassian([None, None, None], &broken, DEFAULT_INSTANCE),
            Err(ResolveError::Failed(_))
        ));
    }

    #[test]
    fn parses_atlassian_login_lines_for_the_values_flags_do_not_cover() {
        assert_eq!(
            parse_atlassian_login("acme.atlassian.net\nuser@example.com\ntoken\n", None, None)
                .unwrap(),
            (
                "acme.atlassian.net".into(),
                "user@example.com".into(),
                "token".into()
            )
        );
        assert_eq!(
            parse_atlassian_login(
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
        assert!(parse_atlassian_login("only-a-token\n", None, None).is_err());
        assert!(parse_atlassian_login("host\nemail\ntoken\nextra\n", None, None).is_err());
        assert!(parse_atlassian_login("host\n\ntoken\n", None, None).is_err());
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
            jira_identity(&account),
            "User <user@example.com>  acme.atlassian.net"
        );
    }
}
