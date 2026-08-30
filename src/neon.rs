//! Neon provider. REST passthrough against console.neon.tech/api/v2 with a
//! bearer API key. Everything but `project list` is scoped to one project,
//! selected with --project or NEON_PROJECT_ID.

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest::{self, Api, Auth, insert_opt, push_query};

const BASE_URL: &str = "https://console.neon.tech";

#[derive(Args)]
pub struct Cmd {
    /// Project ID; defaults to NEON_PROJECT_ID
    #[arg(long, global = true)]
    project: Option<String>,
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Organizations the API key can see
    #[command(subcommand)]
    Org(OrgCmd),
    /// Projects
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Branches
    #[command(subcommand)]
    Branch(BranchCmd),
    /// Databases on a branch
    #[command(subcommand)]
    Database(DatabaseCmd),
    /// Postgres roles on a branch
    #[command(subcommand)]
    Role(RoleCmd),
    /// Compute endpoints
    #[command(subcommand)]
    Endpoint(EndpointCmd),
    /// Long-running operations
    #[command(subcommand)]
    Operation(OperationCmd),
    /// Get a connection URI for a database
    #[command(after_long_help = outdoc::lines(&[
        r#"{"uri": "postgres://..."}"#,
        "Raw Neon data; foac adds no envelope",
    ]))]
    ConnectionUri {
        /// Database name
        #[arg(long)]
        database: String,
        /// Role name
        #[arg(long)]
        role: String,
        /// Branch ID; defaults to the project's default branch
        #[arg(long)]
        branch: Option<String>,
        /// Compute endpoint ID
        #[arg(long)]
        endpoint: Option<String>,
        /// Use the connection pooler
        #[arg(long)]
        pooled: bool,
    },
}

#[derive(Args)]
struct Cursor {
    /// Results per page
    #[arg(long, default_value_t = 50)]
    limit: u32,
    /// Opaque cursor from pageInfo.endCursor
    #[arg(long)]
    after: Option<String>,
}

#[derive(Subcommand)]
enum OrgCmd {
    /// List the current user's organizations
    #[command(after_long_help = outdoc::rest_list("raw Neon organization objects", &[], &outdoc::NEON_SINGLE_PAGE))]
    List,
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// List projects accessible to the API key
    #[command(after_long_help = outdoc::rest_list("raw Neon project objects", &[], &outdoc::END_CURSOR))]
    List {
        /// Organization ID like org-...; defaults to NEON_ORG_ID. Neon
        /// requires it when the account belongs to an organization; find IDs
        /// with `org list`
        #[arg(long)]
        org: Option<String>,
        /// Filter by project name or ID substring
        #[arg(long)]
        search: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get the selected project
    #[command(after_long_help = outdoc::lines(&[
        r#"{"project": {...}}"#,
        "Primary identifier: project.id",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Get,
}

#[derive(Subcommand)]
enum BranchCmd {
    /// List the project's branches
    #[command(after_long_help = outdoc::rest_list("raw Neon branch objects", &["id"], &outdoc::END_CURSOR))]
    List {
        /// Filter by branch name substring
        #[arg(long)]
        search: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a branch by ID like br-...
    #[command(after_long_help = outdoc::lines(&[
        r#"{"branch": {...}}"#,
        "Primary identifier: branch.id",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create a branch
    #[command(after_long_help = outdoc::lines(&[
        r#"{"branch": {...}, "operations": [<operation>, ...]}"#,
        "Primary identifier: branch.id",
        "Asynchronous: poll `operation get ID` for each operation until status is finished",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Create {
        /// Branch name; Neon generates one when omitted
        #[arg(long)]
        name: Option<String>,
        /// Parent branch ID; defaults to the project's default branch
        #[arg(long)]
        parent: Option<String>,
    },
    /// Delete a branch
    #[command(after_long_help = outdoc::lines(&[
        r#"{"branch": {...}, "operations": [<operation>, ...]}"#,
        "Primary identifier: branch.id",
        "Asynchronous: poll `operation get ID` for each operation until status is finished",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Delete { id: String },
}

#[derive(Subcommand)]
enum DatabaseCmd {
    /// List a branch's databases
    #[command(after_long_help = outdoc::rest_list("raw Neon database objects", &[], &outdoc::NEON_SINGLE_PAGE))]
    List {
        /// Branch ID
        #[arg(long)]
        branch: String,
    },
}

#[derive(Subcommand)]
enum RoleCmd {
    /// List a branch's Postgres roles
    #[command(after_long_help = outdoc::rest_list("raw Neon role objects", &[], &outdoc::NEON_SINGLE_PAGE))]
    List {
        /// Branch ID
        #[arg(long)]
        branch: String,
    },
}

#[derive(Subcommand)]
enum EndpointCmd {
    /// List the project's compute endpoints
    #[command(after_long_help = outdoc::rest_list("raw Neon compute endpoint objects", &["id"], &outdoc::NEON_SINGLE_PAGE))]
    List,
    /// Get an endpoint by ID like ep-...
    #[command(after_long_help = outdoc::lines(&[
        r#"{"endpoint": {...}}"#,
        "Primary identifier: endpoint.id",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Start an endpoint
    #[command(after_long_help = outdoc::lines(&[
        r#"{"endpoint": {...}, "operations": [<operation>, ...]}"#,
        "Primary identifier: endpoint.id",
        "Asynchronous: poll `operation get ID` for each operation until status is finished",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Start { id: String },
    /// Suspend an endpoint
    #[command(after_long_help = outdoc::lines(&[
        r#"{"endpoint": {...}, "operations": [<operation>, ...]}"#,
        "Primary identifier: endpoint.id",
        "Asynchronous: poll `operation get ID` for each operation until status is finished",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Suspend { id: String },
    /// Restart an endpoint
    #[command(after_long_help = outdoc::lines(&[
        r#"{"endpoint": {...}, "operations": [<operation>, ...]}"#,
        "Primary identifier: endpoint.id",
        "Asynchronous: poll `operation get ID` for each operation until status is finished",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Restart { id: String },
}

#[derive(Subcommand)]
enum OperationCmd {
    /// List the project's operations, most recent first
    #[command(after_long_help = outdoc::rest_list("raw Neon operation objects", &["id"], &outdoc::END_CURSOR))]
    List {
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get an operation by ID
    #[command(after_long_help = outdoc::lines(&[
        r#"{"operation": {...}}"#,
        "Primary identifier: operation.id",
        "Raw Neon data; foac adds no envelope",
    ]))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["api".to_owned(), "v2".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Cmd { project, command } = cmd;
    let api = api(crate::auth::neon_token(instance)?, format)?;
    match command {
        Resource::Org(cmd) => run_org(&api, cmd),
        Resource::Project(cmd) => run_project(&api, project, cmd),
        Resource::Branch(cmd) => run_branch(&api, selected_project(project)?, cmd),
        Resource::Database(cmd) => run_database(&api, selected_project(project)?, cmd),
        Resource::Role(cmd) => run_role(&api, selected_project(project)?, cmd),
        Resource::Endpoint(cmd) => run_endpoint(&api, selected_project(project)?, cmd),
        Resource::Operation(cmd) => run_operation(&api, selected_project(project)?, cmd),
        Resource::ConnectionUri {
            database,
            role,
            branch,
            endpoint,
            pooled,
        } => run_connection_uri(
            &api,
            selected_project(project)?,
            database,
            role,
            branch,
            endpoint,
            pooled,
        ),
    }
}

fn run_connection_uri(
    api: &Api,
    project: String,
    database: String,
    role: String,
    branch: Option<String>,
    endpoint: Option<String>,
    pooled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut query = vec![("database_name", database), ("role_name", role)];
    push_query(&mut query, "branch_id", branch);
    push_query(&mut query, "endpoint_id", endpoint);
    if pooled {
        query.push(("pooled", "true".to_owned()));
    }
    api.print(
        Method::GET,
        path!["projects", project, "connection_uri"],
        query,
        None,
    )
}

pub fn authenticated() -> bool {
    crate::auth::neon_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("neon")
}

pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    let url = reqwest::Url::parse(&format!("{BASE_URL}/api/v2/users/me"))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        &[],
        &[reqwest::StatusCode::UNAUTHORIZED],
    )
}

fn run_org(api: &Api, cmd: OrgCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        OrgCmd::List => print_list(api, path!["users", "me", "organizations"], "organizations"),
    }
}

fn run_project(
    api: &Api,
    project: Option<String>,
    cmd: ProjectCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ProjectCmd::List {
            org,
            search,
            cursor,
        } => {
            let mut query = Vec::new();
            push_query(&mut query, "org_id", org.or_else(environment_org));
            push_query(&mut query, "search", search);
            print_cursor_list(api, path!["projects"], query, "projects", cursor)
        }
        ProjectCmd::Get => api.print(
            Method::GET,
            path!["projects", selected_project(project)?],
            Vec::new(),
            None,
        ),
    }
}

fn run_branch(
    api: &Api,
    project: String,
    cmd: BranchCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        BranchCmd::List { search, cursor } => {
            let mut query = Vec::new();
            push_query(&mut query, "search", search);
            print_cursor_list(
                api,
                path!["projects", project, "branches"],
                query,
                "branches",
                cursor,
            )
        }
        BranchCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["projects", project, "branches", id], Vec::new())
        }),
        BranchCmd::Create { name, parent } => {
            let mut branch = Map::new();
            insert_opt(&mut branch, "name", name);
            insert_opt(&mut branch, "parent_id", parent);
            api.print(
                Method::POST,
                path!["projects", project, "branches"],
                Vec::new(),
                Some(json!({ "branch": branch })),
            )
        }
        BranchCmd::Delete { id } => api.print(
            Method::DELETE,
            path!["projects", project, "branches", id],
            Vec::new(),
            None,
        ),
    }
}

fn run_database(
    api: &Api,
    project: String,
    cmd: DatabaseCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DatabaseCmd::List { branch } => print_list(
            api,
            path!["projects", project, "branches", branch, "databases"],
            "databases",
        ),
    }
}

fn run_role(api: &Api, project: String, cmd: RoleCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        RoleCmd::List { branch } => print_list(
            api,
            path!["projects", project, "branches", branch, "roles"],
            "roles",
        ),
    }
}

fn run_endpoint(
    api: &Api,
    project: String,
    cmd: EndpointCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        EndpointCmd::List => print_list(api, path!["projects", project, "endpoints"], "endpoints"),
        EndpointCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["projects", project, "endpoints", id], Vec::new())
        }),
        EndpointCmd::Start { id } => endpoint_action(api, &project, &id, "start"),
        EndpointCmd::Suspend { id } => endpoint_action(api, &project, &id, "suspend"),
        EndpointCmd::Restart { id } => endpoint_action(api, &project, &id, "restart"),
    }
}

fn endpoint_action(
    api: &Api,
    project: &str,
    id: &str,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    api.print(
        Method::POST,
        path!["projects", project, "endpoints", id, action],
        Vec::new(),
        None,
    )
}

fn run_operation(
    api: &Api,
    project: String,
    cmd: OperationCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        OperationCmd::List { cursor } => print_cursor_list(
            api,
            path!["projects", project, "operations"],
            Vec::new(),
            "operations",
            cursor,
        ),
        OperationCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["projects", project, "operations", id], Vec::new())
        }),
    }
}

fn api(token: String, format: crate::output::Format) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(BASE_URL)?,
        auth: Auth::Bearer(token),
        format,
        headers: Vec::new(),
        trailing_slash: false,
    })
}

fn environment_org() -> Option<String> {
    std::env::var("NEON_ORG_ID")
        .ok()
        .filter(|org| !org.is_empty())
}

fn selected_project(explicit: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    explicit
        .or_else(|| {
            std::env::var("NEON_PROJECT_ID")
                .ok()
                .filter(|project| !project.is_empty())
        })
        .ok_or_else(|| "--project or NEON_PROJECT_ID is required".into())
}

fn print_cursor_list(
    api: &Api,
    segments: Vec<String>,
    mut query: Vec<(&'static str, String)>,
    key: &str,
    cursor: Cursor,
) -> Result<(), Box<dyn std::error::Error>> {
    query.push(("limit", cursor.limit.to_string()));
    push_query(&mut query, "cursor", cursor.after);
    let response = api.send(Method::GET, &segments, &query, None)?;
    let items = list_items(&response.body, key)?;
    let page_info = page_info(&response.body, items.len(), cursor.limit);
    crate::output::print(&rest::wrap_list(items, page_info), api.format);
    Ok(())
}

fn print_list(
    api: &Api,
    segments: Vec<String>,
    key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = api.send(Method::GET, &segments, &[], None)?;
    let items = list_items(&response.body, key)?;
    crate::output::print(
        &rest::wrap_list(
            items,
            json!({ "hasNextPage": false, "endCursor": Value::Null }),
        ),
        api.format,
    );
    Ok(())
}

fn list_items(body: &Value, key: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    body[key]
        .as_array()
        .cloned()
        .ok_or_else(|| format!("Neon list response did not contain {key}").into())
}

/// Branch lists carry the cursor as `pagination.next`, project and operation
/// lists as `pagination.cursor`; either is present even on the last page (it is the keyset of
/// the last returned item, not a has-more signal), so a full page is the only
/// usable has-more hint; the worst case is one extra empty fetch when the
/// total is an exact multiple of the limit.
fn page_info(body: &Value, count: usize, limit: u32) -> Value {
    let pagination = &body["pagination"];
    let cursor = pagination["next"]
        .as_str()
        .or_else(|| pagination["cursor"].as_str())
        .map(str::to_owned);
    let has_next = cursor.is_some() && count > 0 && count as u64 == u64::from(limit);
    json!({
        "hasNextPage": has_next,
        "endCursor": if has_next { json!(cursor) } else { Value::Null },
    })
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn page_info_uses_the_full_page_heuristic() {
        let body = json!({ "branches": [1, 2], "pagination": { "cursor": "abc" } });
        let more = page_info(&body, 2, 2);
        assert_eq!(more["hasNextPage"], true);
        assert_eq!(more["endCursor"], "abc");

        let short = page_info(&body, 1, 2);
        assert_eq!(short["hasNextPage"], false);
        assert_eq!(short["endCursor"], Value::Null);

        let branches = json!({ "branches": [1, 2], "pagination": { "next": "nxt" } });
        assert_eq!(page_info(&branches, 2, 2)["endCursor"], "nxt");

        assert_eq!(page_info(&json!({}), 2, 2)["hasNextPage"], false);
        assert_eq!(page_info(&body, 0, 0)["hasNextPage"], false);
    }

    #[test]
    fn branch_list_sends_bearer_auth_limit_and_cursor() {
        let (api, request_rx, server) = test_api(
            "200 OK",
            "{\"branches\":[],\"pagination\":{\"cursor\":\"x\"}}",
        );
        run_branch(
            &api,
            "proj-1".into(),
            BranchCmd::List {
                search: Some("preview".into()),
                cursor: Cursor {
                    limit: 25,
                    after: Some("abc".into()),
                },
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /api/v2/projects/proj-1/branches?"));
        assert!(request.contains("search=preview"));
        assert!(request.contains("limit=25"));
        assert!(request.contains("cursor=abc"));
        assert!(request.contains("authorization: bearer "));
        // The token belongs in the header only, never in the URL.
        assert!(!request.lines().next().unwrap().contains("api-key-secret"));
    }

    #[test]
    fn branch_create_sends_the_branch_payload() {
        let (api, request_rx, server) = test_api("201 Created", "{}");
        run_branch(
            &api,
            "proj-1".into(),
            BranchCmd::Create {
                name: Some("preview".into()),
                parent: Some("br-parent".into()),
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /api/v2/projects/proj-1/branches "));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({ "branch": { "name": "preview", "parent_id": "br-parent" } })
        );

        let (api, request_rx, server) = test_api("201 Created", "{}");
        run_branch(
            &api,
            "proj-1".into(),
            BranchCmd::Create {
                name: None,
                parent: None,
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({ "branch": {} })
        );
    }

    #[test]
    fn project_list_sends_the_org_id() {
        let (api, request_rx, server) = test_api(
            "200 OK",
            "{\"projects\":[],\"pagination\":{\"cursor\":\"x\"}}",
        );
        run_project(
            &api,
            None,
            ProjectCmd::List {
                org: Some("org-123".into()),
                search: None,
                cursor: Cursor {
                    limit: 50,
                    after: None,
                },
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /api/v2/projects?"));
        assert!(request.contains("org_id=org-123"));
    }

    #[test]
    fn org_list_reads_the_current_users_organizations() {
        let (api, request_rx, server) = test_api("200 OK", "{\"organizations\":[]}");
        run_org(&api, OrgCmd::List).unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("GET /api/v2/users/me/organizations "));
    }

    #[test]
    fn endpoint_start_posts_without_a_body() {
        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_endpoint(
            &api,
            "proj-1".into(),
            EndpointCmd::Start { id: "ep-1".into() },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /api/v2/projects/proj-1/endpoints/ep-1/start "));
    }

    #[test]
    fn connection_uri_sends_required_and_optional_params() {
        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_connection_uri(
            &api,
            "proj-1".into(),
            "app".into(),
            "app_owner".into(),
            Some("br-main".into()),
            None,
            true,
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /api/v2/projects/proj-1/connection_uri?"));
        assert!(request.contains("database_name=app"));
        assert!(request.contains("role_name=app_owner"));
        assert!(request.contains("branch_id=br-main"));
        assert!(!request.contains("endpoint_id"));
        assert!(request.contains("pooled=true"));

        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_connection_uri(
            &api,
            "proj-1".into(),
            "app".into(),
            "app_owner".into(),
            None,
            None,
            false,
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(!request.contains("pooled"));
    }

    fn test_api(
        status: &str,
        body: &str,
    ) -> (Api, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let (url, request_rx, server) = rest::testing::test_server(status, body, "");
        let api = Api {
            client: reqwest::blocking::Client::new(),
            format: crate::output::Format::Json,
            base_url: url,
            auth: Auth::Bearer("api-key-secret".into()),
            headers: Vec::new(),
            trailing_slash: false,
        };
        (api, request_rx, server)
    }
}
