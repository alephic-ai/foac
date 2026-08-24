//! Jira Cloud provider. Uses REST API v2 (plain-text description and comment
//! bodies rather than v3's Atlassian Document Format) plus the Agile 1.0 API
//! for sprints. Auth is HTTP Basic: account email + API token against the
//! tenant host, shared at the Atlassian vendor level with Confluence.

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::rest::{self, Api, Auth, BodyInput, insert_opt, push_query};

#[derive(Args)]
pub struct Cmd {
    /// Atlassian site host like acme.atlassian.net; defaults to ATLASSIAN_HOST
    #[arg(long, global = true)]
    host: Option<String>,
    /// Atlassian account email; defaults to ATLASSIAN_EMAIL
    #[arg(long, global = true)]
    email: Option<String>,
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Issues
    #[command(subcommand)]
    Issue(IssueCmd),
    /// Issue comments
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Projects
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Sprints on Jira Software boards
    #[command(subcommand)]
    Sprint(SprintCmd),
    /// Users
    #[command(subcommand)]
    User(UserCmd),
    /// Workflow transitions available to an issue
    #[command(subcommand)]
    Transition(TransitionCmd),
}

#[derive(Args)]
struct Page {
    /// Results per page
    #[arg(long, default_value_t = 50)]
    limit: u32,
    /// Zero-based index of the first result
    #[arg(long, default_value_t = 0)]
    start_at: u64,
}

#[derive(Subcommand)]
enum IssueCmd {
    /// List issues matching a JQL query
    List {
        /// JQL query, e.g. "project = ENG AND statusCategory != Done"
        #[arg(long)]
        jql: Option<String>,
        /// Comma-separated fields to return; defaults to *navigable
        #[arg(long)]
        fields: Option<String>,
        /// Results per page
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Opaque page token from pageInfo.nextPageToken
        #[arg(long)]
        after: Option<String>,
    },
    /// Get an issue by key like ENG-123
    Get { key: String },
    /// Create an issue
    Create {
        /// Project key or numeric ID
        #[arg(long)]
        project: String,
        /// Issue type name or numeric ID, e.g. Task
        #[arg(long = "type")]
        issue_type: String,
        #[arg(long)]
        summary: String,
        /// Description via --body or --body-file
        #[command(flatten)]
        body: BodyInput,
        /// Assignee account ID
        #[arg(long)]
        assignee: Option<String>,
        #[arg(long = "label")]
        labels: Vec<String>,
        /// Priority name or numeric ID
        #[arg(long)]
        priority: Option<String>,
        /// Parent issue key
        #[arg(long)]
        parent: Option<String>,
    },
    /// Update an issue; only supplied fields are changed
    Update {
        key: String,
        #[arg(long)]
        summary: Option<String>,
        /// Description via --body or --body-file
        #[command(flatten)]
        body: BodyInput,
        /// Assignee account ID
        #[arg(long)]
        assignee: Option<String>,
        /// Replace all labels
        #[arg(long = "label")]
        labels: Vec<String>,
        /// Priority name or numeric ID
        #[arg(long)]
        priority: Option<String>,
    },
    /// Move an issue through a workflow transition
    Transition {
        key: String,
        /// Transition ID, transition name, or destination status name
        #[arg(long)]
        to: String,
    },
}

#[derive(Subcommand)]
enum CommentCmd {
    /// List an issue's comments
    List {
        /// Issue key like ENG-123
        #[arg(long)]
        issue: String,
        #[command(flatten)]
        page: Page,
    },
    /// Add a comment to an issue
    Create {
        /// Issue key like ENG-123
        #[arg(long)]
        issue: String,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Update a comment
    Update {
        id: String,
        /// Issue key like ENG-123
        #[arg(long)]
        issue: String,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Delete a comment
    Delete {
        id: String,
        /// Issue key like ENG-123
        #[arg(long)]
        issue: String,
    },
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// List projects visible to the account
    List {
        /// Match project names and keys
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a project by key or numeric ID
    Get { key: String },
}

#[derive(Subcommand)]
enum SprintCmd {
    /// List a board's sprints
    List {
        /// Board numeric ID or exact board name
        #[arg(long)]
        board: String,
        /// future, active, or closed
        #[arg(long)]
        state: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a sprint by numeric ID
    Get { id: u64 },
}

#[derive(Subcommand)]
enum UserCmd {
    /// List users, or search them with --query
    List {
        /// Match display names and email addresses
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a user by account ID
    Get { account_id: String },
}

#[derive(Subcommand)]
enum TransitionCmd {
    /// List the transitions available to an issue
    List {
        /// Issue key like ENG-123
        #[arg(long)]
        issue: String,
    },
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["rest".to_owned(), "api".to_owned(), "2".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

macro_rules! agile_path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["rest".to_owned(), "agile".to_owned(), "1.0".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    let Cmd {
        host,
        email,
        command,
    } = cmd;
    let api = api(crate::auth::jira_credentials(host, email)?, format)?;
    match command {
        Resource::Issue(cmd) => run_issue(&api, cmd),
        Resource::Comment(cmd) => run_comment(&api, cmd),
        Resource::Project(cmd) => run_project(&api, cmd),
        Resource::Sprint(cmd) => run_sprint(&api, cmd),
        Resource::User(cmd) => run_user(&api, cmd),
        Resource::Transition(cmd) => run_transition(&api, cmd),
    }
}

pub fn authenticated() -> bool {
    crate::auth::jira_authenticated()
}

pub(crate) fn auth_identity(
    host: &str,
    email: &str,
    token: &str,
) -> Result<Value, crate::auth::ValidationError> {
    let url = reqwest::Url::parse(&format!("https://{host}/rest/api/2/myself"))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    auth_identity_at(email, token, url)
}

fn auth_identity_at(
    email: &str,
    token: &str,
    url: reqwest::Url,
) -> Result<Value, crate::auth::ValidationError> {
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

fn run_issue(api: &Api, cmd: IssueCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        IssueCmd::List {
            jql,
            fields,
            limit,
            after,
        } => {
            let mut query = vec![
                ("maxResults", limit.to_string()),
                // The search endpoint returns only IDs by default.
                ("fields", fields.unwrap_or_else(|| "*navigable".to_owned())),
            ];
            push_query(&mut query, "jql", jql);
            push_query(&mut query, "nextPageToken", after);
            let response = api.send(Method::GET, &path!["search", "jql"], &query, None)?;
            let items = list_items(&response.body, Some("issues"))?;
            crate::output::print(
                &rest::wrap_list(items, token_page_info(&response.body)),
                api.format,
            );
            Ok(())
        }
        IssueCmd::Get { key } => api.print(Method::GET, path!["issue", key], Vec::new(), None),
        IssueCmd::Create {
            project,
            issue_type,
            summary,
            body,
            assignee,
            labels,
            priority,
            parent,
        } => {
            let mut fields = Map::new();
            fields.insert("project".into(), key_or_id(project));
            fields.insert("issuetype".into(), name_or_id(issue_type));
            fields.insert("summary".into(), summary.into());
            insert_opt(&mut fields, "description", body.read()?);
            insert_issue_fields(&mut fields, assignee, labels, priority);
            if let Some(parent) = parent {
                fields.insert("parent".into(), json!({ "key": parent }));
            }
            api.print(
                Method::POST,
                path!["issue"],
                Vec::new(),
                Some(json!({ "fields": fields })),
            )
        }
        IssueCmd::Update {
            key,
            summary,
            body,
            assignee,
            labels,
            priority,
        } => {
            let mut fields = Map::new();
            insert_opt(&mut fields, "summary", summary);
            insert_opt(&mut fields, "description", body.read()?);
            insert_issue_fields(&mut fields, assignee, labels, priority);
            api.print(
                Method::PUT,
                path!["issue", key],
                Vec::new(),
                Some(json!({ "fields": fields })),
            )
        }
        IssueCmd::Transition { key, to } => {
            let id = resolve_transition_id(api, &key, &to)?;
            api.print(
                Method::POST,
                path!["issue", key, "transitions"],
                Vec::new(),
                Some(json!({ "transition": { "id": id } })),
            )
        }
    }
}

fn run_comment(api: &Api, cmd: CommentCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommentCmd::List { issue, page } => print_offset_list(
            api,
            path!["issue", issue, "comment"],
            Vec::new(),
            page,
            Some("comments"),
        ),
        CommentCmd::Create { issue, body } => api.print(
            Method::POST,
            path!["issue", issue, "comment"],
            Vec::new(),
            Some(json!({ "body": body.required()? })),
        ),
        CommentCmd::Update { id, issue, body } => api.print(
            Method::PUT,
            path!["issue", issue, "comment", id],
            Vec::new(),
            Some(json!({ "body": body.required()? })),
        ),
        CommentCmd::Delete { id, issue } => api.print(
            Method::DELETE,
            path!["issue", issue, "comment", id],
            Vec::new(),
            None,
        ),
    }
}

fn run_project(api: &Api, cmd: ProjectCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ProjectCmd::List { query, page } => {
            let mut parameters = Vec::new();
            push_query(&mut parameters, "query", query);
            print_offset_list(
                api,
                path!["project", "search"],
                parameters,
                page,
                Some("values"),
            )
        }
        ProjectCmd::Get { key } => api.print(Method::GET, path!["project", key], Vec::new(), None),
    }
}

fn run_sprint(api: &Api, cmd: SprintCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        SprintCmd::List { board, state, page } => {
            let board = resolve_board_id(api, &board)?;
            let mut parameters = Vec::new();
            push_query(&mut parameters, "state", state);
            print_offset_list(
                api,
                agile_path!["board", board, "sprint"],
                parameters,
                page,
                Some("values"),
            )
        }
        SprintCmd::Get { id } => api.print(Method::GET, agile_path!["sprint", id], Vec::new(), None),
    }
}

fn run_user(api: &Api, cmd: UserCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        UserCmd::List { query, page } => match query {
            Some(query) => print_offset_list(
                api,
                path!["user", "search"],
                vec![("query", query)],
                page,
                None,
            ),
            None => print_offset_list(api, path!["users", "search"], Vec::new(), page, None),
        },
        UserCmd::Get { account_id } => api.print(
            Method::GET,
            path!["user"],
            vec![("accountId", account_id)],
            None,
        ),
    }
}

fn run_transition(api: &Api, cmd: TransitionCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        TransitionCmd::List { issue } => {
            let response = api.send(
                Method::GET,
                &path!["issue", issue, "transitions"],
                &[],
                None,
            )?;
            let items = list_items(&response.body, Some("transitions"))?;
            crate::output::print(
                &rest::wrap_list(items, json!({ "hasNextPage": false })),
                api.format,
            );
            Ok(())
        }
    }
}

fn api(
    credentials: crate::auth::JiraCredentials,
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
        headers: &[],
        trailing_slash: false,
    })
}

fn print_offset_list(
    api: &Api,
    segments: Vec<String>,
    mut query: Vec<(&'static str, String)>,
    page: Page,
    key: Option<&'static str>,
) -> Result<(), Box<dyn std::error::Error>> {
    query.push(("startAt", page.start_at.to_string()));
    query.push(("maxResults", page.limit.to_string()));
    let response = api.send(Method::GET, &segments, &query, None)?;
    let items = list_items(&response.body, key)?;
    let page_info = offset_page_info(page.start_at, page.limit, items.len(), &response.body);
    crate::output::print(&rest::wrap_list(items, page_info), api.format);
    Ok(())
}

fn list_items(body: &Value, key: Option<&str>) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let items = match key {
        None => body.as_array(),
        Some(key) => body[key].as_array(),
    };
    items
        .cloned()
        .ok_or_else(|| "Jira list response did not contain the expected array".into())
}

fn token_page_info(body: &Value) -> Value {
    let next = body["nextPageToken"].as_str();
    json!({
        "hasNextPage": next.is_some(),
        "nextPageToken": next,
    })
}

/// Jira's offset-paged endpoints report `total` and/or `isLast`; the user
/// search endpoints report neither, so a full page means there may be more.
fn offset_page_info(start_at: u64, limit: u32, count: usize, body: &Value) -> Value {
    let next = start_at + count as u64;
    let has_next = match (
        body.get("total").and_then(Value::as_u64),
        body.get("isLast").and_then(Value::as_bool),
    ) {
        (Some(total), _) => next < total,
        (None, Some(is_last)) => !is_last,
        (None, None) => count > 0 && count as u64 == u64::from(limit),
    };
    json!({
        "hasNextPage": has_next,
        "nextStartAt": if has_next { json!(next) } else { Value::Null },
    })
}

fn insert_issue_fields(
    fields: &mut Map<String, Value>,
    assignee: Option<String>,
    labels: Vec<String>,
    priority: Option<String>,
) {
    if let Some(assignee) = assignee {
        fields.insert("assignee".into(), json!({ "accountId": assignee }));
    }
    if !labels.is_empty() {
        fields.insert("labels".into(), labels.into());
    }
    if let Some(priority) = priority {
        fields.insert("priority".into(), name_or_id(priority));
    }
}

fn key_or_id(value: String) -> Value {
    if is_numeric_id(&value) {
        json!({ "id": value })
    } else {
        json!({ "key": value })
    }
}

fn name_or_id(value: String) -> Value {
    if is_numeric_id(&value) {
        json!({ "id": value })
    } else {
        json!({ "name": value })
    }
}

fn is_numeric_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn resolve_transition_id(
    api: &Api,
    key: &str,
    to: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if is_numeric_id(to) {
        return Ok(to.to_owned());
    }
    let response = api.send(Method::GET, &path!["issue", key, "transitions"], &[], None)?;
    find_transition(&response.body, to).ok_or_else(|| {
        format!("could not resolve transition {to}; run `foac jira transition list --issue {key}`")
            .into()
    })
}

fn find_transition(body: &Value, name: &str) -> Option<String> {
    body["transitions"]
        .as_array()?
        .iter()
        .find(|transition| {
            let matches = |value: &Value| {
                value
                    .as_str()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            };
            matches(&transition["name"]) || matches(&transition["to"]["name"])
        })
        .and_then(|transition| id_string(&transition["id"]))
}

fn resolve_board_id(api: &Api, board: &str) -> Result<String, Box<dyn std::error::Error>> {
    if is_numeric_id(board) {
        return Ok(board.to_owned());
    }
    let response = api.send(
        Method::GET,
        &agile_path!["board"],
        &[("name", board.to_owned())],
        None,
    )?;
    let values = response.body["values"].as_array().cloned().unwrap_or_default();
    values
        .iter()
        .find(|candidate| candidate["name"].as_str() == Some(board))
        .or_else(|| (values.len() == 1).then(|| &values[0]))
        .and_then(|candidate| id_string(&candidate["id"]))
        .ok_or_else(|| format!("could not resolve board {board}").into())
}

fn id_string(value: &Value) -> Option<String> {
    match value {
        Value::String(id) => Some(id.clone()),
        Value::Number(id) => Some(id.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn normalizes_hosts_and_rejects_empty_input() {
        assert_eq!(normalize_host(" acme.atlassian.net \n").unwrap(), "acme.atlassian.net");
        assert_eq!(
            normalize_host("https://acme.atlassian.net/").unwrap(),
            "acme.atlassian.net"
        );
        assert!(normalize_host(" \n").is_err());
        assert!(normalize_host("https:///").is_err());
    }

    #[test]
    fn distinguishes_numeric_ids_from_keys_and_names() {
        assert_eq!(key_or_id("ENG".into()), json!({ "key": "ENG" }));
        assert_eq!(key_or_id("10001".into()), json!({ "id": "10001" }));
        assert_eq!(name_or_id("Task".into()), json!({ "name": "Task" }));
        assert_eq!(name_or_id("3".into()), json!({ "id": "3" }));
        assert!(!is_numeric_id(""));
    }

    #[test]
    fn token_page_info_follows_next_page_token() {
        let info = token_page_info(&json!({ "issues": [], "nextPageToken": "abc" }));
        assert_eq!(info["hasNextPage"], true);
        assert_eq!(info["nextPageToken"], "abc");
        let info = token_page_info(&json!({ "issues": [] }));
        assert_eq!(info["hasNextPage"], false);
        assert_eq!(info["nextPageToken"], Value::Null);
    }

    #[test]
    fn offset_page_info_uses_total_then_is_last_then_a_full_page_heuristic() {
        let total = offset_page_info(0, 2, 2, &json!({ "total": 5 }));
        assert_eq!(total["hasNextPage"], true);
        assert_eq!(total["nextStartAt"], 2);
        let done = offset_page_info(3, 2, 2, &json!({ "total": 5 }));
        assert_eq!(done["hasNextPage"], false);
        assert_eq!(done["nextStartAt"], Value::Null);

        let is_last = offset_page_info(0, 2, 2, &json!({ "isLast": false }));
        assert_eq!(is_last["hasNextPage"], true);
        assert_eq!(offset_page_info(0, 2, 2, &json!({ "isLast": true }))["hasNextPage"], false);

        let bare_full = offset_page_info(0, 2, 2, &json!({}));
        assert_eq!(bare_full["hasNextPage"], true);
        assert_eq!(offset_page_info(0, 2, 1, &json!({}))["hasNextPage"], false);
        assert_eq!(offset_page_info(0, 0, 0, &json!({}))["hasNextPage"], false);
    }

    #[test]
    fn finds_transitions_by_transition_or_status_name() {
        let body = json!({ "transitions": [
            { "id": "11", "name": "Start work", "to": { "name": "In Progress" } },
            { "id": 21, "name": "Close", "to": { "name": "Done" } },
        ]});
        assert_eq!(find_transition(&body, "start work").unwrap(), "11");
        assert_eq!(find_transition(&body, "done").unwrap(), "21");
        assert!(find_transition(&body, "Reopen").is_none());
        assert!(find_transition(&json!({}), "Done").is_none());
    }

    #[test]
    fn issue_search_sends_basic_auth_jql_and_default_fields() {
        let (api, request_rx, server) = test_api("200 OK", "{\"issues\":[]}");
        run_issue(
            &api,
            IssueCmd::List {
                jql: Some("project = ENG".into()),
                fields: None,
                limit: 25,
                after: None,
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /rest/api/2/search/jql?"));
        assert!(request.contains("maxresults=25"));
        assert!(request.contains("fields=*navigable"));
        assert!(request.contains("jql=project"));
        assert!(request.contains("authorization: basic "));
        assert!(!request.contains("api-token"));
    }

    #[test]
    fn issue_create_nests_fields_with_plain_text_description() {
        let (api, request_rx, server) = test_api("201 Created", "{}");
        run_issue(
            &api,
            IssueCmd::Create {
                project: "ENG".into(),
                issue_type: "Task".into(),
                summary: "Fix login".into(),
                body: BodyInput {
                    body: Some("Steps to reproduce".into()),
                    body_file: None,
                },
                assignee: Some("5b10a2844c20165700ede21g".into()),
                labels: vec!["auth".into()],
                priority: None,
                parent: None,
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /rest/api/2/issue "));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({ "fields": {
                "project": { "key": "ENG" },
                "issuetype": { "name": "Task" },
                "summary": "Fix login",
                "description": "Steps to reproduce",
                "assignee": { "accountId": "5b10a2844c20165700ede21g" },
                "labels": ["auth"],
            }})
        );
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
            auth: Auth::Basic {
                user: "user@example.com".into(),
                password: "api-token".into(),
            },
            headers: &[],
            trailing_slash: false,
        };
        (api, request_rx, server)
    }
}
