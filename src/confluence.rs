//! Confluence Cloud provider. Uses REST API v2 for spaces, pages, and footer
//! comments, plus the v1 search endpoint for CQL (never ported to v2). Bodies
//! are written as wiki markup and read back in the storage representation.
//! Auth is HTTP Basic: account email + API token against the tenant host,
//! shared at the Atlassian vendor level with Jira.

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::rest::{self, Api, Auth, BodyInput, id_string, insert_opt, is_numeric_id, push_query};

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
    /// Spaces
    #[command(subcommand)]
    Space(SpaceCmd),
    /// Pages
    #[command(subcommand)]
    Page(PageCmd),
    /// Footer comments on pages
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Search content with CQL
    Search {
        /// CQL query, e.g. 'type = page AND text ~ "login"'
        #[arg(long)]
        cql: String,
        /// Results per page
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Zero-based index of the first result
        #[arg(long, default_value_t = 0)]
        start_at: u64,
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
enum SpaceCmd {
    /// List spaces
    List {
        /// Filter by space key; repeat for multiple
        #[arg(long = "key")]
        keys: Vec<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a space by key or numeric ID
    Get { space: String },
}

#[derive(Subcommand)]
enum PageCmd {
    /// List pages
    List {
        /// Space key or numeric ID
        #[arg(long)]
        space: Option<String>,
        /// Exact page title
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a page by numeric ID, body in storage representation
    Get { id: String },
    /// Create a page
    Create {
        /// Space key or numeric ID
        #[arg(long)]
        space: String,
        #[arg(long)]
        title: String,
        /// Parent page numeric ID
        #[arg(long)]
        parent: Option<String>,
        /// Content in Confluence wiki markup via --body or --body-file
        #[command(flatten)]
        body: BodyInput,
    },
    /// Update a page; fetches the current version and keeps omitted fields
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        /// Content in Confluence wiki markup via --body or --body-file
        #[command(flatten)]
        body: BodyInput,
    },
    /// Delete a page
    Delete { id: String },
}

#[derive(Subcommand)]
enum CommentCmd {
    /// List a page's footer comments
    List {
        /// Page numeric ID
        #[arg(long)]
        page: String,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Add a footer comment to a page
    Create {
        /// Page numeric ID
        #[arg(long)]
        page: String,
        /// Content in Confluence wiki markup via --body or --body-file
        #[command(flatten)]
        body: BodyInput,
    },
    /// Update a footer comment; fetches the current version internally
    Update {
        id: String,
        /// Content in Confluence wiki markup via --body or --body-file
        #[command(flatten)]
        body: BodyInput,
    },
    /// Delete a footer comment
    Delete { id: String },
}

macro_rules! v2_path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["wiki".to_owned(), "api".to_owned(), "v2".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

/// CQL search was never ported to v2, so it stays on the v1 root.
macro_rules! v1_path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["wiki".to_owned(), "rest".to_owned(), "api".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Cmd {
        host,
        email,
        command,
    } = cmd;
    let api = api(
        crate::auth::atlassian_credentials(host, email, "foac auth confluence login", instance)?,
        format,
    )?;
    match command {
        Resource::Space(cmd) => run_space(&api, cmd),
        Resource::Page(cmd) => run_page(&api, cmd),
        Resource::Comment(cmd) => run_comment(&api, cmd),
        Resource::Search {
            cql,
            limit,
            start_at,
        } => run_search(&api, cql, limit, start_at),
    }
}

pub fn authenticated() -> bool {
    crate::auth::atlassian_authenticated()
}

pub(crate) fn auth_identity(
    host: &str,
    email: &str,
    token: &str,
) -> Result<Value, crate::auth::ValidationError> {
    let url = reqwest::Url::parse(&format!("https://{host}/wiki/rest/api/user/current"))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
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

fn run_space(api: &Api, cmd: SpaceCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        SpaceCmd::List { keys, cursor } => {
            let mut query = Vec::new();
            if !keys.is_empty() {
                query.push(("keys", keys.join(",")));
            }
            print_cursor_list(api, v2_path!["spaces"], query, cursor)
        }
        SpaceCmd::Get { space } => {
            let id = resolve_space_id(api, &space)?;
            api.print(Method::GET, v2_path!["spaces", id], Vec::new(), None)
        }
    }
}

fn run_page(api: &Api, cmd: PageCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PageCmd::List {
            space,
            title,
            cursor,
        } => {
            let mut query = Vec::new();
            if let Some(space) = space {
                query.push(("space-id", resolve_space_id(api, &space)?));
            }
            push_query(&mut query, "title", title);
            print_cursor_list(api, v2_path!["pages"], query, cursor)
        }
        PageCmd::Get { id } => api.print(
            Method::GET,
            v2_path!["pages", id],
            vec![("body-format", "storage".to_owned())],
            None,
        ),
        PageCmd::Create {
            space,
            title,
            parent,
            body,
        } => {
            let space_id = resolve_space_id(api, &space)?;
            let mut payload = Map::new();
            payload.insert("spaceId".into(), space_id.into());
            payload.insert("status".into(), "current".into());
            payload.insert("title".into(), title.into());
            insert_opt(&mut payload, "parentId", parent);
            if let Some(body) = body.read()? {
                payload.insert("body".into(), wiki_body(body));
            }
            api.print(
                Method::POST,
                v2_path!["pages"],
                Vec::new(),
                Some(Value::Object(payload)),
            )
        }
        PageCmd::Update { id, title, body } => {
            let current = api.send(
                Method::GET,
                &v2_path!["pages", id],
                &[("body-format", "storage".to_owned())],
                None,
            )?;
            let payload = page_update_payload(&current.body, &id, title, body.read()?)?;
            api.print(
                Method::PUT,
                v2_path!["pages", id],
                Vec::new(),
                Some(payload),
            )
        }
        PageCmd::Delete { id } => {
            api.print(Method::DELETE, v2_path!["pages", id], Vec::new(), None)
        }
    }
}

fn run_comment(api: &Api, cmd: CommentCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommentCmd::List { page, cursor } => print_cursor_list(
            api,
            v2_path!["pages", page, "footer-comments"],
            vec![("body-format", "storage".to_owned())],
            cursor,
        ),
        CommentCmd::Create { page, body } => api.print(
            Method::POST,
            v2_path!["footer-comments"],
            Vec::new(),
            Some(json!({ "pageId": page, "body": wiki_body(body.required()?) })),
        ),
        CommentCmd::Update { id, body } => {
            let current = api.send(Method::GET, &v2_path!["footer-comments", id], &[], None)?;
            let payload = comment_update_payload(&current.body, body.required()?)?;
            api.print(
                Method::PUT,
                v2_path!["footer-comments", id],
                Vec::new(),
                Some(payload),
            )
        }
        CommentCmd::Delete { id } => api.print(
            Method::DELETE,
            v2_path!["footer-comments", id],
            Vec::new(),
            None,
        ),
    }
}

fn run_search(
    api: &Api,
    cql: String,
    limit: u32,
    start_at: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = vec![
        ("cql", cql),
        ("start", start_at.to_string()),
        ("limit", limit.to_string()),
    ];
    let response = api.send(Method::GET, &v1_path!["search"], &query, None)?;
    let items = list_items(&response.body)?;
    let page_info = search_page_info(start_at, limit, items.len(), &response.body);
    crate::output::print(&rest::wrap_list(items, page_info), api.format);
    Ok(())
}

fn api(
    credentials: crate::auth::AtlassianCredentials,
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

fn wiki_body(value: String) -> Value {
    json!({ "representation": "wiki", "value": value })
}

/// The v2 PUT requires status, title, and body re-sent with the incremented
/// version. Wiki markup is write-only, so an unchanged body round-trips in
/// the storage representation it was fetched in.
fn page_update_payload(
    current: &Value,
    id: &str,
    title: Option<String>,
    body: Option<String>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let version = current["version"]["number"]
        .as_u64()
        .ok_or("Confluence did not return the current page version")?;
    let title = match title {
        Some(title) => title,
        None => current["title"]
            .as_str()
            .ok_or("Confluence did not return the current page title")?
            .to_owned(),
    };
    let body = match body {
        Some(value) => wiki_body(value),
        None => json!({
            "representation": "storage",
            "value": current["body"]["storage"]["value"]
                .as_str()
                .ok_or("Confluence did not return the current page body")?,
        }),
    };
    Ok(json!({
        "id": id,
        "status": current["status"].as_str().unwrap_or("current"),
        "title": title,
        "body": body,
        "version": { "number": version + 1 },
    }))
}

fn comment_update_payload(
    current: &Value,
    body: String,
) -> Result<Value, Box<dyn std::error::Error>> {
    let version = current["version"]["number"]
        .as_u64()
        .ok_or("Confluence did not return the current comment version")?;
    Ok(json!({
        "version": { "number": version + 1 },
        "body": wiki_body(body),
    }))
}

fn resolve_space_id(api: &Api, space: &str) -> Result<String, Box<dyn std::error::Error>> {
    if is_numeric_id(space) {
        return Ok(space.to_owned());
    }
    let response = api.send(
        Method::GET,
        &v2_path!["spaces"],
        &[("keys", space.to_owned())],
        None,
    )?;
    response.body["results"]
        .as_array()
        .and_then(|results| results.first())
        .and_then(|result| id_string(&result["id"]))
        .ok_or_else(|| format!("could not resolve space {space}").into())
}

fn print_cursor_list(
    api: &Api,
    segments: Vec<String>,
    mut query: Vec<(&'static str, String)>,
    cursor: Cursor,
) -> Result<(), Box<dyn std::error::Error>> {
    query.push(("limit", cursor.limit.to_string()));
    push_query(&mut query, "cursor", cursor.after);
    let response = api.send(Method::GET, &segments, &query, None)?;
    let items = list_items(&response.body)?;
    crate::output::print(
        &rest::wrap_list(items, cursor_page_info(&response.body)),
        api.format,
    );
    Ok(())
}

fn list_items(body: &Value) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    body["results"]
        .as_array()
        .cloned()
        .ok_or_else(|| "Confluence list response did not contain the expected array".into())
}

/// v2 lists link the next page as a URL in `_links.next`; its `cursor` query
/// parameter is the opaque token `--after` takes.
fn cursor_page_info(body: &Value) -> Value {
    let cursor = body["_links"]["next"].as_str().and_then(next_cursor);
    json!({
        "hasNextPage": cursor.is_some(),
        "endCursor": cursor,
    })
}

fn next_cursor(next: &str) -> Option<String> {
    // `next` is usually site-relative like /wiki/api/v2/pages?cursor=...
    let base = reqwest::Url::parse("https://placeholder.invalid").ok()?;
    let url = base.join(next).ok()?;
    url.query_pairs()
        .find(|(name, _)| name == "cursor")
        .map(|(_, value)| value.into_owned())
}

/// The v1 search root is offset-paged and reports `totalSize`; without it a
/// full page means there may be more.
fn search_page_info(start_at: u64, limit: u32, count: usize, body: &Value) -> Value {
    let next = start_at + count as u64;
    let has_next = match body.get("totalSize").and_then(Value::as_u64) {
        Some(total) => next < total,
        None => count > 0 && count as u64 == u64::from(limit),
    };
    json!({
        "hasNextPage": has_next,
        "nextStartAt": if has_next { json!(next) } else { Value::Null },
    })
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn cursor_page_info_extracts_the_cursor_from_the_next_link() {
        let info = cursor_page_info(&json!({
            "results": [],
            "_links": { "next": "/wiki/api/v2/pages?cursor=abc%3D%3D&limit=25" },
        }));
        assert_eq!(info["hasNextPage"], true);
        assert_eq!(info["endCursor"], "abc==");

        let absolute = cursor_page_info(&json!({
            "_links": { "next": "https://acme.atlassian.net/wiki/api/v2/spaces?cursor=xyz" },
        }));
        assert_eq!(absolute["endCursor"], "xyz");

        let done = cursor_page_info(&json!({ "results": [], "_links": {} }));
        assert_eq!(done["hasNextPage"], false);
        assert_eq!(done["endCursor"], Value::Null);
    }

    #[test]
    fn search_page_info_uses_total_size_then_a_full_page_heuristic() {
        let more = search_page_info(0, 2, 2, &json!({ "totalSize": 5 }));
        assert_eq!(more["hasNextPage"], true);
        assert_eq!(more["nextStartAt"], 2);
        let done = search_page_info(3, 2, 2, &json!({ "totalSize": 5 }));
        assert_eq!(done["hasNextPage"], false);
        assert_eq!(done["nextStartAt"], Value::Null);

        assert_eq!(search_page_info(0, 2, 2, &json!({}))["hasNextPage"], true);
        assert_eq!(search_page_info(0, 2, 1, &json!({}))["hasNextPage"], false);
        assert_eq!(search_page_info(0, 0, 0, &json!({}))["hasNextPage"], false);
    }

    #[test]
    fn page_update_keeps_omitted_fields_and_increments_the_version() {
        let current = json!({
            "id": "123",
            "status": "current",
            "title": "Old title",
            "version": { "number": 4 },
            "body": { "storage": { "representation": "storage", "value": "<p>old</p>" } },
        });

        let unchanged_body =
            page_update_payload(&current, "123", Some("New title".into()), None).unwrap();
        assert_eq!(
            unchanged_body,
            json!({
                "id": "123",
                "status": "current",
                "title": "New title",
                "body": { "representation": "storage", "value": "<p>old</p>" },
                "version": { "number": 5 },
            })
        );

        let new_body = page_update_payload(&current, "123", None, Some("h1. New".into())).unwrap();
        assert_eq!(new_body["title"], "Old title");
        assert_eq!(
            new_body["body"],
            json!({ "representation": "wiki", "value": "h1. New" })
        );

        assert!(page_update_payload(&json!({}), "123", None, None).is_err());
    }

    #[test]
    fn comment_update_increments_the_version_and_sends_wiki_markup() {
        let payload = comment_update_payload(
            &json!({ "id": "7", "version": { "number": 2 } }),
            "Done, see PR #42".into(),
        )
        .unwrap();
        assert_eq!(
            payload,
            json!({
                "version": { "number": 3 },
                "body": { "representation": "wiki", "value": "Done, see PR #42" },
            })
        );
        assert!(comment_update_payload(&json!({}), "text".into()).is_err());
    }

    #[test]
    fn space_ids_pass_through_and_keys_resolve_via_the_api() {
        let (api, request_rx, server) = test_api(
            "200 OK",
            "{\"results\":[{\"id\":\"98765\",\"key\":\"ENG\"}]}",
        );
        assert_eq!(resolve_space_id(&api, "12345").unwrap(), "12345");
        assert_eq!(resolve_space_id(&api, "ENG").unwrap(), "98765");
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("GET /wiki/api/v2/spaces?keys=ENG "));
    }

    #[test]
    fn page_list_sends_basic_auth_limit_and_cursor() {
        let (api, request_rx, server) = test_api("200 OK", "{\"results\":[]}");
        run_page(
            &api,
            PageCmd::List {
                space: Some("123".into()),
                title: None,
                cursor: Cursor {
                    limit: 25,
                    after: Some("abc".into()),
                },
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /wiki/api/v2/pages?"));
        assert!(request.contains("space-id=123"));
        assert!(request.contains("limit=25"));
        assert!(request.contains("cursor=abc"));
        assert!(request.contains("authorization: basic "));
        assert!(!request.contains("api-token"));
    }

    #[test]
    fn comment_create_sends_the_wiki_representation() {
        let (api, request_rx, server) = test_api("201 Created", "{}");
        run_comment(
            &api,
            CommentCmd::Create {
                page: "123".into(),
                body: BodyInput {
                    body: Some("Looks good".into()),
                    body_file: None,
                },
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /wiki/api/v2/footer-comments "));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({
                "pageId": "123",
                "body": { "representation": "wiki", "value": "Looks good" },
            })
        );
    }

    #[test]
    fn search_uses_the_v1_root_with_offset_paging() {
        let (api, request_rx, server) = test_api("200 OK", "{\"results\":[],\"totalSize\":0}");
        run_search(&api, "type = page".into(), 25, 50).unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /wiki/rest/api/search?"));
        assert!(request.contains("cql=type"));
        assert!(request.contains("start=50"));
        assert!(request.contains("limit=25"));
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
