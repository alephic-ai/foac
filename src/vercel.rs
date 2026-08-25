//! Vercel provider. REST passthrough against api.vercel.com with a bearer
//! access token. Team-owned resources are selected with --team or
//! VERCEL_TEAM_ID; omitting both uses the token's personal account.

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::rest::{self, Api, Auth, insert_opt, push_query};

const BASE_URL: &str = "https://api.vercel.com";

#[derive(Args)]
pub struct Cmd {
    /// Team ID like team_...; defaults to VERCEL_TEAM_ID, otherwise personal scope
    #[arg(long, global = true)]
    team: Option<String>,
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Teams visible to the access token
    #[command(subcommand)]
    Team(TeamCmd),
    /// Projects
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Deployments
    #[command(subcommand)]
    Deployment(DeploymentCmd),
    /// Domains owned by the account
    #[command(subcommand)]
    Domain(DomainCmd),
    /// Domains assigned to projects
    #[command(subcommand)]
    ProjectDomain(ProjectDomainCmd),
}

#[derive(Args)]
struct Cursor {
    /// Results per page
    #[arg(long, default_value_t = 20)]
    limit: u32,
    /// Cursor from pageInfo.endCursor
    #[arg(long)]
    after: Option<String>,
}

#[derive(Subcommand)]
enum TeamCmd {
    /// List teams visible to the access token
    List {
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a team by ID like team_...
    Get { id: String },
}

#[derive(Args, Default)]
struct ProjectOpts {
    /// Framework preset, e.g. nextjs, vite, or rust
    #[arg(long)]
    framework: Option<String>,
    /// Project root directory within the repository
    #[arg(long)]
    root_directory: Option<String>,
    /// Build command
    #[arg(long)]
    build_command: Option<String>,
    /// Development command
    #[arg(long)]
    dev_command: Option<String>,
    /// Dependency installation command
    #[arg(long)]
    install_command: Option<String>,
    /// Build output directory
    #[arg(long)]
    output_directory: Option<String>,
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// List projects
    List {
        /// Search project names
        #[arg(long)]
        search: Option<String>,
        /// Filter by connected repository name
        #[arg(long)]
        repo: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a project by ID or name
    Get { id: String },
    /// Create a project
    Create {
        /// Project name
        #[arg(long)]
        name: String,
        #[command(flatten)]
        opts: ProjectOpts,
    },
    /// Update a project; only supplied fields are changed
    Update {
        /// Project ID or name
        id: String,
        /// Rename the project
        #[arg(long)]
        name: Option<String>,
        /// Node.js version, e.g. 22.x
        #[arg(long)]
        node_version: Option<String>,
        #[command(flatten)]
        opts: ProjectOpts,
    },
    /// Delete a project
    Delete { id: String },
}

#[derive(Subcommand)]
enum DeploymentCmd {
    /// List deployments
    List {
        /// Filter by project ID or name
        #[arg(long)]
        project: Option<String>,
        /// Filter by state, e.g. READY, ERROR, BUILDING, or CANCELED
        #[arg(long)]
        state: Option<String>,
        /// Filter by target environment, e.g. production or preview
        #[arg(long)]
        target: Option<String>,
        /// Filter by Git branch
        #[arg(long)]
        branch: Option<String>,
        /// Filter by Git commit SHA
        #[arg(long)]
        sha: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a deployment by ID or URL
    Get { id: String },
    /// Cancel a deployment
    Cancel { id: String },
    /// Delete a deployment
    Delete { id: String },
}

#[derive(Subcommand)]
enum DomainCmd {
    /// List domains owned by the account
    List {
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a domain by name
    Get { name: String },
    /// Get the DNS configuration Vercel expects for a domain
    Config {
        name: String,
        /// Use project-specific configuration
        #[arg(long)]
        project: Option<String>,
        /// Return a strict configuration assessment
        #[arg(long)]
        strict: bool,
    },
    /// Add an existing domain to the account
    Create {
        name: String,
        /// Whether to create a Vercel DNS zone
        #[arg(long)]
        zone: Option<bool>,
        /// Whether to enable the Vercel Edge Network
        #[arg(long)]
        cdn_enabled: Option<bool>,
    },
    /// Remove a domain from the account
    Delete { name: String },
}

#[derive(Args, Default)]
struct ProjectDomainOpts {
    /// Git branch to assign to the domain
    #[arg(long)]
    git_branch: Option<String>,
    /// Destination domain for a redirect
    #[arg(long)]
    redirect: Option<String>,
    /// Redirect status code: 301, 302, 307, or 308
    #[arg(long, value_parser = parse_redirect_status)]
    redirect_status: Option<u16>,
}

#[derive(Subcommand)]
enum ProjectDomainCmd {
    /// List a project's domains
    List {
        /// Project ID or name
        #[arg(long)]
        project: String,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a project domain
    Get {
        #[arg(long)]
        project: String,
        name: String,
    },
    /// Add a domain to a project
    Create {
        #[arg(long)]
        project: String,
        name: String,
        #[command(flatten)]
        opts: ProjectDomainOpts,
    },
    /// Update a project domain; only supplied fields are changed
    Update {
        #[arg(long)]
        project: String,
        name: String,
        #[command(flatten)]
        opts: ProjectDomainOpts,
    },
    /// Remove a domain from a project
    Delete {
        #[arg(long)]
        project: String,
        name: String,
        /// Also remove project domains that redirect to this domain
        #[arg(long)]
        remove_redirects: bool,
    },
    /// Ask Vercel to verify a project domain's ownership challenge
    Verify {
        #[arg(long)]
        project: String,
        name: String,
    },
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        vec![$($segment.to_string()),*]
    }};
}

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    let Cmd { team, command } = cmd;
    let team = team.or_else(environment_team);
    let api = api(crate::auth::vercel_token()?, format)?;
    match command {
        Resource::Team(cmd) => run_team(&api, cmd),
        Resource::Project(cmd) => run_project(&api, team.as_deref(), cmd),
        Resource::Deployment(cmd) => run_deployment(&api, team.as_deref(), cmd),
        Resource::Domain(cmd) => run_domain(&api, team.as_deref(), cmd),
        Resource::ProjectDomain(cmd) => run_project_domain(&api, team.as_deref(), cmd),
    }
}

pub fn authenticated() -> bool {
    crate::auth::vercel_token().is_ok()
}

pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    auth_identity_at(token, BASE_URL)
}

fn auth_identity_at(token: &str, base_url: &str) -> Result<Value, crate::auth::ValidationError> {
    let url = reqwest::Url::parse(&format!("{}/v2/user", base_url.trim_end_matches('/')))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    let response = rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        &[],
        &[
            reqwest::StatusCode::UNAUTHORIZED,
            reqwest::StatusCode::FORBIDDEN,
        ],
    )?;
    response
        .get("user")
        .cloned()
        .ok_or_else(|| crate::auth::ValidationError::Failed("Vercel response has no user".into()))
}

fn run_team(api: &Api, cmd: TeamCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        TeamCmd::List { cursor } => print_list(
            api,
            path!["v2", "teams"],
            list_query(None, "until", cursor),
            "teams",
        ),
        TeamCmd::Get { id } => api.print(Method::GET, path!["v2", "teams", id], Vec::new(), None),
    }
}

fn run_project(
    api: &Api,
    team: Option<&str>,
    cmd: ProjectCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ProjectCmd::List {
            search,
            repo,
            cursor,
        } => {
            let mut query = list_query(team, "from", cursor);
            push_query(&mut query, "search", search);
            push_query(&mut query, "repo", repo);
            print_list(api, path!["v10", "projects"], query, "projects")
        }
        ProjectCmd::Get { id } => api.print(
            Method::GET,
            path!["v9", "projects", id],
            team_query(team),
            None,
        ),
        ProjectCmd::Create { name, opts } => {
            let mut payload = project_payload(opts);
            payload.insert("name".into(), name.into());
            api.print(
                Method::POST,
                path!["v11", "projects"],
                team_query(team),
                Some(payload.into()),
            )
        }
        ProjectCmd::Update {
            id,
            name,
            node_version,
            opts,
        } => {
            let mut payload = project_payload(opts);
            insert_opt(&mut payload, "name", name);
            insert_opt(&mut payload, "nodeVersion", node_version);
            api.print(
                Method::PATCH,
                path!["v9", "projects", id],
                team_query(team),
                Some(payload.into()),
            )
        }
        ProjectCmd::Delete { id } => api.print(
            Method::DELETE,
            path!["v9", "projects", id],
            team_query(team),
            None,
        ),
    }
}

fn run_deployment(
    api: &Api,
    team: Option<&str>,
    cmd: DeploymentCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DeploymentCmd::List {
            project,
            state,
            target,
            branch,
            sha,
            cursor,
        } => {
            let mut query = list_query(team, "until", cursor);
            push_query(&mut query, "projectId", project);
            push_query(&mut query, "state", state);
            push_query(&mut query, "target", target);
            push_query(&mut query, "branch", branch);
            push_query(&mut query, "sha", sha);
            print_list(api, path!["v7", "deployments"], query, "deployments")
        }
        DeploymentCmd::Get { id } => api.print(
            Method::GET,
            path!["v13", "deployments", id],
            team_query(team),
            None,
        ),
        DeploymentCmd::Cancel { id } => api.print(
            Method::PATCH,
            path!["v12", "deployments", id, "cancel"],
            team_query(team),
            None,
        ),
        DeploymentCmd::Delete { id } => api.print(
            Method::DELETE,
            path!["v13", "deployments", id],
            team_query(team),
            None,
        ),
    }
}

fn run_domain(
    api: &Api,
    team: Option<&str>,
    cmd: DomainCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DomainCmd::List { cursor } => print_list(
            api,
            path!["v5", "domains"],
            list_query(team, "until", cursor),
            "domains",
        ),
        DomainCmd::Get { name } => api.print(
            Method::GET,
            path!["v5", "domains", name],
            team_query(team),
            None,
        ),
        DomainCmd::Config {
            name,
            project,
            strict,
        } => {
            let mut query = team_query(team);
            push_query(&mut query, "projectIdOrName", project);
            if strict {
                query.push(("strict", "true".into()));
            }
            api.print(
                Method::GET,
                path!["v6", "domains", name, "config"],
                query,
                None,
            )
        }
        DomainCmd::Create {
            name,
            zone,
            cdn_enabled,
        } => {
            let mut payload = Map::new();
            payload.insert("name".into(), name.into());
            payload.insert("method".into(), "add".into());
            insert_opt(&mut payload, "zone", zone);
            insert_opt(&mut payload, "cdnEnabled", cdn_enabled);
            api.print(
                Method::POST,
                path!["v7", "domains"],
                team_query(team),
                Some(payload.into()),
            )
        }
        DomainCmd::Delete { name } => api.print(
            Method::DELETE,
            path!["v6", "domains", name],
            team_query(team),
            None,
        ),
    }
}

fn run_project_domain(
    api: &Api,
    team: Option<&str>,
    cmd: ProjectDomainCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ProjectDomainCmd::List { project, cursor } => print_list(
            api,
            path!["v9", "projects", project, "domains"],
            list_query(team, "until", cursor),
            "domains",
        ),
        ProjectDomainCmd::Get { project, name } => api.print(
            Method::GET,
            path!["v9", "projects", project, "domains", name],
            team_query(team),
            None,
        ),
        ProjectDomainCmd::Create {
            project,
            name,
            opts,
        } => {
            let mut payload = project_domain_payload(opts);
            payload.insert("name".into(), name.into());
            api.print(
                Method::POST,
                path!["v10", "projects", project, "domains"],
                team_query(team),
                Some(payload.into()),
            )
        }
        ProjectDomainCmd::Update {
            project,
            name,
            opts,
        } => api.print(
            Method::PATCH,
            path!["v9", "projects", project, "domains", name],
            team_query(team),
            Some(project_domain_payload(opts).into()),
        ),
        ProjectDomainCmd::Delete {
            project,
            name,
            remove_redirects,
        } => {
            let body = remove_redirects.then(|| json!({ "removeRedirects": true }));
            api.print(
                Method::DELETE,
                path!["v9", "projects", project, "domains", name],
                team_query(team),
                body,
            )
        }
        ProjectDomainCmd::Verify { project, name } => api.print(
            Method::POST,
            path!["v9", "projects", project, "domains", name, "verify"],
            team_query(team),
            None,
        ),
    }
}

fn project_payload(opts: ProjectOpts) -> Map<String, Value> {
    let mut payload = Map::new();
    insert_opt(&mut payload, "framework", opts.framework);
    insert_opt(&mut payload, "rootDirectory", opts.root_directory);
    insert_opt(&mut payload, "buildCommand", opts.build_command);
    insert_opt(&mut payload, "devCommand", opts.dev_command);
    insert_opt(&mut payload, "installCommand", opts.install_command);
    insert_opt(&mut payload, "outputDirectory", opts.output_directory);
    payload
}

fn parse_redirect_status(value: &str) -> Result<u16, String> {
    match value.parse::<u16>() {
        Ok(status @ (301 | 302 | 307 | 308)) => Ok(status),
        _ => Err("redirect status must be one of 301, 302, 307, or 308".into()),
    }
}

fn project_domain_payload(opts: ProjectDomainOpts) -> Map<String, Value> {
    let mut payload = Map::new();
    insert_opt(&mut payload, "gitBranch", opts.git_branch);
    insert_opt(&mut payload, "redirect", opts.redirect);
    insert_opt(&mut payload, "redirectStatusCode", opts.redirect_status);
    payload
}

fn api(token: String, format: crate::output::Format) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(BASE_URL)?,
        auth: Auth::Bearer(token),
        format,
        headers: &[],
        trailing_slash: false,
    })
}

fn environment_team() -> Option<String> {
    std::env::var("VERCEL_TEAM_ID")
        .ok()
        .filter(|team| !team.is_empty())
}

fn team_query(team: Option<&str>) -> Vec<(&'static str, String)> {
    let mut query = Vec::new();
    push_query(&mut query, "teamId", team);
    query
}

fn list_query(
    team: Option<&str>,
    cursor_name: &'static str,
    cursor: Cursor,
) -> Vec<(&'static str, String)> {
    let mut query = team_query(team);
    query.push(("limit", cursor.limit.to_string()));
    push_query(&mut query, cursor_name, cursor.after);
    query
}

fn print_list(
    api: &Api,
    segments: Vec<String>,
    query: Vec<(&'static str, String)>,
    key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = api.send(Method::GET, &segments, &query, None)?;
    let items = list_items(&response.body, key)?;
    crate::output::print(
        &rest::wrap_list(items, page_info(&response.body)),
        api.format,
    );
    Ok(())
}

fn list_items(body: &Value, key: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let items = body
        .get(key)
        .or_else(|| body.as_array().map(|_| body))
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Vercel response has no {key} array"))?;
    Ok(items.clone())
}

fn page_info(body: &Value) -> Value {
    let next = body
        .get("pagination")
        .and_then(|pagination| pagination.get("next"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "hasNextPage": !next.is_null(),
        "endCursor": next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest::testing::test_server;

    fn test_api(
        status: &str,
        body: &str,
    ) -> (
        Api,
        std::sync::mpsc::Receiver<String>,
        std::thread::JoinHandle<()>,
    ) {
        let (base_url, request_rx, server) = test_server(status, body, "");
        (
            Api {
                client: reqwest::blocking::Client::new(),
                base_url,
                auth: Auth::Bearer("vercel-token-secret".into()),
                format: crate::output::Format::Json,
                headers: &[],
                trailing_slash: false,
            },
            request_rx,
            server,
        )
    }

    fn request_body(request: &str) -> Value {
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn translates_vercel_timestamp_pagination() {
        assert_eq!(
            page_info(&json!({ "pagination": { "count": 2, "next": 123, "prev": null } })),
            json!({ "hasNextPage": true, "endCursor": 123 })
        );
        assert_eq!(
            page_info(&json!({ "pagination": { "count": 1, "next": null, "prev": 123 } })),
            json!({ "hasNextPage": false, "endCursor": null })
        );
        assert_eq!(
            page_info(&json!({})),
            json!({ "hasNextPage": false, "endCursor": null })
        );
    }

    #[test]
    fn project_list_sends_team_search_limit_and_cursor() {
        let (api, request_rx, server) = test_api(
            "200 OK",
            r#"{"projects":[],"pagination":{"count":0,"next":null,"prev":null}}"#,
        );
        run_project(
            &api,
            Some("team_123"),
            ProjectCmd::List {
                search: Some("docs".into()),
                repo: Some("alephic-ai/foac".into()),
                cursor: Cursor {
                    limit: 30,
                    after: Some("cursor-1".into()),
                },
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /v10/projects?"));
        assert!(request.contains("teamid=team_123"));
        assert!(request.contains("search=docs"));
        assert!(request.contains("repo=alephic-ai%2ffoac"));
        assert!(request.contains("limit=30"));
        assert!(request.contains("from=cursor-1"));
        assert!(request.contains("authorization: bearer vercel-token-secret"));
        assert!(
            !request
                .lines()
                .next()
                .unwrap()
                .contains("vercel-token-secret")
        );
    }

    #[test]
    fn project_create_and_update_send_only_supplied_fields() {
        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_project(
            &api,
            None,
            ProjectCmd::Create {
                name: "web".into(),
                opts: ProjectOpts {
                    framework: Some("nextjs".into()),
                    root_directory: Some("apps/web".into()),
                    ..ProjectOpts::default()
                },
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v11/projects "));
        assert_eq!(
            request_body(&request),
            json!({ "name": "web", "framework": "nextjs", "rootDirectory": "apps/web" })
        );

        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_project(
            &api,
            Some("team_123"),
            ProjectCmd::Update {
                id: "web".into(),
                name: Some("site".into()),
                node_version: Some("22.x".into()),
                opts: ProjectOpts::default(),
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("PATCH /v9/projects/web?teamId=team_123 "));
        assert_eq!(
            request_body(&request),
            json!({ "name": "site", "nodeVersion": "22.x" })
        );
    }

    #[test]
    fn deployment_cancel_uses_its_endpoint_version() {
        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_deployment(
            &api,
            Some("team_123"),
            DeploymentCmd::Cancel {
                id: "dpl_123".into(),
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("PATCH /v12/deployments/dpl_123/cancel?teamId=team_123 "));
    }

    #[test]
    fn account_domain_create_sends_add_payload() {
        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_domain(
            &api,
            None,
            DomainCmd::Create {
                name: "example.com".into(),
                zone: Some(true),
                cdn_enabled: Some(false),
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v7/domains "));
        assert_eq!(
            request_body(&request),
            json!({
                "name": "example.com",
                "method": "add",
                "zone": true,
                "cdnEnabled": false
            })
        );
    }

    #[test]
    fn project_domain_create_and_delete_send_native_payloads() {
        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_project_domain(
            &api,
            Some("team_123"),
            ProjectDomainCmd::Create {
                project: "web".into(),
                name: "preview.example.com".into(),
                opts: ProjectDomainOpts {
                    git_branch: Some("feature".into()),
                    redirect: None,
                    redirect_status: None,
                },
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v10/projects/web/domains?teamId=team_123 "));
        assert_eq!(
            request_body(&request),
            json!({ "name": "preview.example.com", "gitBranch": "feature" })
        );

        let (api, request_rx, server) = test_api("200 OK", "{}");
        run_project_domain(
            &api,
            None,
            ProjectDomainCmd::Delete {
                project: "web".into(),
                name: "old.example.com".into(),
                remove_redirects: true,
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("DELETE /v9/projects/web/domains/old.example.com "));
        assert_eq!(request_body(&request), json!({ "removeRedirects": true }));
    }

    #[test]
    fn validates_vercel_identity_and_rejects_bad_credentials() {
        let (base_url, request_rx, server) = test_server(
            "200 OK",
            r#"{"user":{"id":"user_123","username":"lolo","name":"Lolo","email":"lolo@example.com"}}"#,
            "",
        );
        let identity = auth_identity_at("vercel-token-secret", base_url.as_str()).unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /v2/user "));
        assert!(request.contains("authorization: bearer vercel-token-secret"));
        assert_eq!(identity["id"], "user_123");

        let (base_url, _, server) = test_server(
            "403 Forbidden",
            r#"{"error":{"code":"forbidden","message":"invalid token"}}"#,
            "",
        );
        let error = auth_identity_at("bad-token", base_url.as_str()).unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Rejected(_)));
    }

    #[test]
    fn accepts_only_vercel_redirect_status_codes() {
        for status in ["301", "302", "307", "308"] {
            assert_eq!(parse_redirect_status(status).unwrap().to_string(), status);
        }
        for status in ["300", "303", "306", "309", "not-a-status"] {
            assert!(parse_redirect_status(status).is_err());
        }
    }
}
