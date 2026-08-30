use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest::{self, Api, Auth, insert_opt, push_query};

pub(crate) const DEFAULT_HOST: &str = "sentry.io";
const DEFAULT_URL: &str = "https://sentry.io";

#[derive(Args)]
pub struct Cmd {
    /// Organization slug or numeric ID; defaults to SENTRY_ORG
    #[arg(long, global = true)]
    org: Option<String>,
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Organizations
    #[command(subcommand)]
    Org(OrgCmd),
    /// Projects
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Issues (grouped errors)
    #[command(subcommand)]
    Issue(IssueCmd),
    /// Error events
    #[command(subcommand)]
    Event(EventCmd),
    /// Releases
    #[command(subcommand)]
    Release(ReleaseCmd),
}

#[derive(Args)]
struct Page {
    /// Opaque pagination cursor from pageInfo
    #[arg(long)]
    cursor: Option<String>,
}

#[derive(Subcommand)]
enum OrgCmd {
    /// List organizations accessible to the token
    #[command(after_long_help = outdoc::rest_list("raw Sentry organization objects", &[], &outdoc::SENTRY_CURSOR))]
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get the selected organization
    #[command(after_long_help = outdoc::rest_obj("raw Sentry organization object", "slug"))]
    Get,
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// List the organization's projects
    #[command(after_long_help = outdoc::rest_list("raw Sentry project objects", &["slug"], &outdoc::SENTRY_CURSOR))]
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get a project by slug or numeric ID
    #[command(after_long_help = outdoc::rest_obj("raw Sentry project object", "slug"))]
    Get {
        slug: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
enum IssueCmd {
    /// List issues across the organization or one project
    #[command(after_long_help = outdoc::rest_list("raw Sentry issue objects", &["id", "shortId"], &outdoc::SENTRY_CURSOR))]
    List {
        /// Project slug or numeric ID
        #[arg(long)]
        project: Option<String>,
        /// Sentry issue search query, e.g. "is:unresolved release:1.2.0"
        #[arg(long)]
        query: Option<String>,
        /// Relative time range, e.g. 24h or 14d
        #[arg(long)]
        stats_period: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get an issue by numeric ID or short ID like PROJ-123
    #[command(after_long_help = outdoc::rest_obj("raw Sentry issue object", "id (numeric); shortId is the human key like PROJ-123"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Update an issue; only supplied fields are changed
    #[command(after_long_help = outdoc::rest_obj("raw Sentry issue object with the update applied", "id (numeric)"))]
    Update {
        id: String,
        /// resolved, unresolved, or ignored
        #[arg(long)]
        status: Option<String>,
        /// archived_until_escalating, archived_forever, ...
        #[arg(long)]
        substatus: Option<String>,
        /// Username, actor ID, or team like "#team-slug"
        #[arg(long)]
        assigned_to: Option<String>,
    },
    /// Get the most recent event in an issue
    #[command(after_long_help = outdoc::rest_obj("raw Sentry event object", "eventID"))]
    LatestEvent { id: String },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct EventScope {
    /// Issue by numeric ID or short ID
    #[arg(long)]
    issue: Option<String>,
    /// Project slug
    #[arg(long)]
    project: Option<String>,
}

#[derive(Subcommand)]
enum EventCmd {
    /// List events in an issue or a project
    #[command(after_long_help = outdoc::rest_list("raw Sentry event objects", &["eventID"], &outdoc::SENTRY_CURSOR))]
    List {
        #[command(flatten)]
        scope: EventScope,
        #[command(flatten)]
        page: Page,
    },
    /// Get an event by ID
    #[command(after_long_help = outdoc::rest_obj("raw Sentry event object", "eventID"))]
    Get {
        id: Option<String>,
        /// Project slug or numeric ID
        #[arg(long)]
        project: String,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
enum ReleaseCmd {
    /// List the organization's releases
    #[command(after_long_help = outdoc::rest_list("raw Sentry release objects", &["version"], &outdoc::SENTRY_CURSOR))]
    List {
        /// Filter versions by substring
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a release by version
    #[command(after_long_help = outdoc::rest_obj("raw Sentry release object", "version"))]
    Get {
        version: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["api".to_owned(), "0".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Cmd { org, command } = cmd;
    let api = api(crate::auth::sentry_token(instance)?, format, instance)?;
    match command {
        Resource::Org(cmd) => run_org(&api, org, cmd),
        Resource::Project(cmd) => run_project(&api, selected_org(org)?, cmd),
        Resource::Issue(cmd) => run_issue(&api, selected_org(org)?, cmd),
        Resource::Event(cmd) => run_event(&api, selected_org(org)?, cmd),
        Resource::Release(cmd) => run_release(&api, selected_org(org)?, cmd),
    }
}

pub fn authenticated() -> bool {
    crate::auth::sentry_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("sentry")
}

pub(crate) fn auth_identity(
    token: &str,
    url_override: Option<&str>,
    instance: &str,
) -> Result<Value, crate::auth::ValidationError> {
    let base = match url_override {
        Some(url) => url.trim_end_matches('/').to_owned(),
        None => base_url(instance),
    };
    let url = reqwest::Url::parse(&format!("{base}/api/0/organizations/"))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    auth_identity_at(token, url)
}

fn auth_identity_at(token: &str, url: reqwest::Url) -> Result<Value, crate::auth::ValidationError> {
    rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        &[],
        &[reqwest::StatusCode::UNAUTHORIZED],
    )
}

fn run_org(api: &Api, org: Option<String>, cmd: OrgCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        OrgCmd::List { page } => {
            print_list(api, Method::GET, path!["organizations"], page_query(page))
        }
        OrgCmd::Get => api.print(
            Method::GET,
            path!["organizations", selected_org(org)?],
            Vec::new(),
            None,
        ),
    }
}

fn run_project(api: &Api, org: String, cmd: ProjectCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ProjectCmd::List { page } => print_list(
            api,
            Method::GET,
            path!["organizations", org, "projects"],
            page_query(page),
        ),
        ProjectCmd::Get { slug, from } => pipe::run_get(slug, from, api.format, |slug| {
            api.get_body(path!["projects", org, slug], Vec::new())
        }),
    }
}

fn run_issue(api: &Api, org: String, cmd: IssueCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        IssueCmd::List {
            project,
            query,
            stats_period,
            page,
        } => {
            let segments = if let Some(project) = project {
                path!["projects", org, project, "issues"]
            } else {
                path!["organizations", org, "issues"]
            };
            let mut parameters = page_query(page);
            push_query(&mut parameters, "query", query);
            push_query(&mut parameters, "statsPeriod", stats_period);
            print_list(api, Method::GET, segments, parameters)
        }
        IssueCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            let id = resolve_issue_id(api, &org, &id)?;
            api.get_body(path!["organizations", org, "issues", id], Vec::new())
        }),
        IssueCmd::Update {
            id,
            status,
            substatus,
            assigned_to,
        } => {
            let id = resolve_issue_id(api, &org, &id)?;
            let mut payload = Map::new();
            insert_opt(&mut payload, "status", status);
            insert_opt(&mut payload, "substatus", substatus);
            insert_opt(&mut payload, "assignedTo", assigned_to);
            api.print(
                Method::PUT,
                path!["organizations", org, "issues", id],
                Vec::new(),
                Some(payload.into()),
            )
        }
        IssueCmd::LatestEvent { id } => {
            let id = resolve_issue_id(api, &org, &id)?;
            api.print(
                Method::GET,
                path!["organizations", org, "issues", id, "events", "latest"],
                Vec::new(),
                None,
            )
        }
    }
}

fn run_event(api: &Api, org: String, cmd: EventCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        EventCmd::List { scope, page } => {
            let segments = match (scope.issue, scope.project) {
                (Some(issue), None) => {
                    let id = resolve_issue_id(api, &org, &issue)?;
                    path!["organizations", org, "issues", id, "events"]
                }
                (None, Some(project)) => path!["projects", org, project, "events"],
                _ => unreachable!("clap enforces --issue xor --project"),
            };
            print_list(api, Method::GET, segments, page_query(page))
        }
        EventCmd::Get { id, project, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["projects", org, project, "events", id], Vec::new())
        }),
    }
}

fn run_release(api: &Api, org: String, cmd: ReleaseCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ReleaseCmd::List { query, page } => {
            let mut parameters = page_query(page);
            push_query(&mut parameters, "query", query);
            print_list(
                api,
                Method::GET,
                path!["organizations", org, "releases"],
                parameters,
            )
        }
        ReleaseCmd::Get { version, from } => pipe::run_get(version, from, api.format, |version| {
            api.get_body(path!["organizations", org, "releases", version], Vec::new())
        }),
    }
}

fn api(
    token: String,
    format: crate::output::Format,
    instance: &str,
) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(&base_url(instance))?,
        auth: Auth::Bearer(token),
        format,
        headers: &[],
        // Sentry requires the trailing slash; without it the API redirects
        // and some proxies drop the request body.
        trailing_slash: true,
    })
}

fn print_list(
    api: &Api,
    method: Method,
    segments: Vec<String>,
    query: Vec<(&'static str, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = api.send(method, &segments, &query, None)?;
    let items = response
        .body
        .as_array()
        .cloned()
        .ok_or("Sentry list response was not an array")?;
    crate::output::print(
        &rest::wrap_list(items, page_info(response.link.as_deref())),
        api.format,
    );
    Ok(())
}

/// The instance's base URL: `SENTRY_URL` (default instance only), then the
/// URL stored with the instance's credentials, then sentry.io.
pub(crate) fn base_url(instance: &str) -> String {
    let environment = if instance == crate::provider::DEFAULT_INSTANCE {
        std::env::var("SENTRY_URL").ok()
    } else {
        None
    };
    resolve_base_url(
        environment,
        crate::auth::stored_url(crate::provider::Credential::SentryUrl, instance),
    )
}

/// Turn the hostname the user gave at login into a base URL: empty means the
/// default host, a pasted scheme is dropped, and the result is always https.
pub(crate) fn normalize_host(input: &str) -> String {
    let host = input.trim().trim_end_matches('/');
    let host = host.split_once("://").map_or(host, |(_, host)| host);
    // Sentry docs and the Terraform provider write the base URL as `https://host/api/`.
    let host = host
        .strip_suffix("/api/0")
        .or_else(|| host.strip_suffix("/api"))
        .unwrap_or(host);
    if host.is_empty() {
        DEFAULT_URL.to_owned()
    } else {
        format!("https://{host}")
    }
}

fn resolve_base_url(environment: Option<String>, stored: Option<String>) -> String {
    environment
        .filter(|url| !url.is_empty())
        .or(stored)
        .map(|url| normalize_host(&url))
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

fn selected_org(explicit: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    explicit
        .or_else(|| {
            std::env::var("SENTRY_ORG")
                .ok()
                .filter(|org| !org.is_empty())
        })
        .ok_or_else(|| "--org or SENTRY_ORG is required".into())
}

fn resolve_issue_id(api: &Api, org: &str, id: &str) -> Result<String, Box<dyn std::error::Error>> {
    if is_numeric_id(id) {
        return Ok(id.to_owned());
    }
    let response = api.send(
        Method::GET,
        &path!["organizations", org, "shortids", id],
        &[],
        None,
    )?;
    group_id(&response.body).ok_or_else(|| format!("could not resolve issue short ID {id}").into())
}

fn is_numeric_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())
}

fn group_id(body: &Value) -> Option<String> {
    match &body["groupId"] {
        Value::String(id) => Some(id.clone()),
        Value::Number(id) => Some(id.to_string()),
        _ => None,
    }
}

fn page_query(page: Page) -> Vec<(&'static str, String)> {
    let mut query = Vec::new();
    push_query(&mut query, "cursor", page.cursor);
    query
}

fn page_info(link: Option<&str>) -> Value {
    let next = link_cursor(link, "next");
    let previous = link_cursor(link, "previous");
    json!({
        "hasNextPage": next.is_some(),
        "nextCursor": next,
        "hasPreviousPage": previous.is_some(),
        "previousCursor": previous,
    })
}

/// A Sentry Link header entry looks like
/// `<url>; rel="next"; results="true"; cursor="0:100:0"`; the cursor is only
/// usable when `results` is true.
fn link_cursor(link: Option<&str>, relation: &str) -> Option<String> {
    link?.split(',').find_map(|part| {
        let mut matches_relation = false;
        let mut has_results = false;
        let mut cursor = None;
        for attribute in part.trim().split(';').skip(1) {
            let attribute = attribute.trim();
            if attribute == format!("rel=\"{relation}\"") {
                matches_relation = true;
            } else if attribute == "results=\"true\"" {
                has_results = true;
            } else if let Some(value) = attribute
                .strip_prefix("cursor=\"")
                .and_then(|value| value.strip_suffix('"'))
            {
                cursor = Some(value.to_owned());
            }
        }
        (matches_relation && has_results)
            .then_some(cursor)
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cursors_from_sentry_link_headers() {
        let link = concat!(
            "<https://sentry.io/api/0/projects/acme/backend/issues/?&cursor=100:0:1>; rel=\"previous\"; results=\"false\"; cursor=\"100:0:1\", ",
            "<https://sentry.io/api/0/projects/acme/backend/issues/?&cursor=100:100:0>; rel=\"next\"; results=\"true\"; cursor=\"100:100:0\""
        );
        let info = page_info(Some(link));
        assert_eq!(info["hasNextPage"], true);
        assert_eq!(info["nextCursor"], "100:100:0");
        assert_eq!(info["hasPreviousPage"], false);
        assert_eq!(info["previousCursor"], Value::Null);
        assert_eq!(page_info(None)["hasNextPage"], false);
    }

    #[test]
    fn normalizes_login_prompt_hosts() {
        assert_eq!(normalize_host("\n"), DEFAULT_URL);
        assert_eq!(
            normalize_host(" sentry.example.com \n"),
            "https://sentry.example.com"
        );
        for pasted_url in [
            "https://sentry.example.com/\n",
            "http://sentry.example.com",
            "https://sentry.example.com/api/",
            "https://sentry.example.com/api/0",
        ] {
            assert_eq!(normalize_host(pasted_url), "https://sentry.example.com");
        }
    }

    #[test]
    fn base_url_prefers_environment_then_stored_then_default() {
        assert_eq!(
            resolve_base_url(Some("https://env.example.com/".into()), Some("x".into())),
            "https://env.example.com"
        );
        assert_eq!(
            resolve_base_url(Some("http://env.example.com/".into()), None),
            "https://env.example.com"
        );
        assert_eq!(
            resolve_base_url(
                Some(String::new()),
                Some("http://stored.example.com".into())
            ),
            "https://stored.example.com"
        );
        assert_eq!(resolve_base_url(None, None), DEFAULT_URL);
    }

    #[test]
    fn distinguishes_numeric_ids_from_short_ids() {
        assert!(is_numeric_id("123456"));
        assert!(!is_numeric_id("PROJ-123"));
        assert!(!is_numeric_id(""));
    }

    #[test]
    fn reads_group_ids_as_strings_or_numbers() {
        assert_eq!(group_id(&json!({ "groupId": "42" })).unwrap(), "42");
        assert_eq!(group_id(&json!({ "groupId": 42 })).unwrap(), "42");
        assert!(group_id(&json!({})).is_none());
    }
}
