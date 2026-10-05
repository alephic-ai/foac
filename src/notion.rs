//! Notion provider. REST passthrough against api.notion.com/v1 with a bearer
//! integration token and a pinned `Notion-Version`. Page content goes in and
//! out as Notion's enhanced Markdown; blocks stay Notion-native JSON.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest::{self, Api, Auth, BodyInput, insert_opt, read_json};

const BASE_URL: &str = "https://api.notion.com";
/// 2025-09-03 split databases into data sources; 2026-03-11 replaced
/// `archived` with `in_trash` and `after` with `position`.
const VERSION: &str = "2026-03-11";

#[derive(Args)]
pub struct Cmd {
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Pages
    #[command(subcommand)]
    Page(PageCmd),
    /// Databases: containers holding one or more data sources
    #[command(subcommand)]
    Database(DatabaseCmd),
    /// Data sources: the tables inside a database, whose rows are pages
    #[command(subcommand)]
    DataSource(DataSourceCmd),
    /// Blocks: page content as Notion-native JSON
    #[command(subcommand)]
    Block(BlockCmd),
    /// Comments
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Search page and data source titles shared with the integration
    #[command(after_long_help = outdoc::rest_list("raw Notion page and data source objects", &["id"], &outdoc::END_CURSOR))]
    Search {
        /// Title text to match; omit to list everything shared
        query: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Users
    #[command(subcommand)]
    User(UserCmd),
}

#[derive(Args)]
struct Cursor {
    /// Results per page (max 100)
    #[arg(long, default_value_t = 50)]
    limit: u32,
    /// Opaque cursor from pageInfo.endCursor
    #[arg(long)]
    after: Option<String>,
}

/// A raw JSON flag given inline or as `--<name>-file`.
#[derive(Args)]
struct PropertiesInput {
    /// Page properties as a JSON object, keyed by property name or ID
    #[arg(long, conflicts_with = "properties_file")]
    properties: Option<String>,
    /// Read the properties JSON object from a file
    #[arg(long, conflicts_with = "properties")]
    properties_file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum PageCmd {
    /// List pages shared with the integration (a title search)
    #[command(after_long_help = outdoc::rest_list("raw Notion page objects", &["id"], &outdoc::END_CURSOR))]
    List {
        /// Title text to match
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a page's properties, or its content with --markdown
    #[command(after_long_help = outdoc::rest_obj("raw Notion page object; with --markdown, the page_markdown object (content in markdown)", "id"))]
    Get {
        /// Page ID
        id: Option<String>,
        /// Return the page content as enhanced Markdown instead of properties
        #[arg(long)]
        markdown: bool,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create a page under a page, a data source, or (personal tokens only) the workspace
    #[command(after_long_help = outdoc::rest_obj("raw Notion page object", "id"))]
    Create {
        #[command(flatten)]
        parent: PageParent,
        /// Page title
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        properties: PropertiesInput,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Update a page; only supplied fields change, and a body replaces all content
    #[command(after_long_help = outdoc::rest_obj("raw Notion page object; with --body, the page_markdown object after the content update", "id"))]
    Update {
        /// Page ID
        id: String,
        /// Page title
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        properties: PropertiesInput,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Move a page to the trash
    #[command(after_long_help = outdoc::rest_obj("raw Notion page object with in_trash: true", "id"))]
    Delete {
        /// Page ID
        id: String,
    },
}

/// Neither flag creates a private workspace-level page, which only personal
/// access tokens and public integrations may do.
#[derive(Args)]
#[group(multiple = false)]
struct PageParent {
    /// Parent page ID
    #[arg(long)]
    parent: Option<String>,
    /// Parent data source ID: the page becomes a row
    #[arg(long)]
    data_source: Option<String>,
}

#[derive(Subcommand)]
enum DatabaseCmd {
    /// Get a database, including the IDs of its data sources
    #[command(after_long_help = outdoc::rest_obj("raw Notion database object", "id; data_sources[].id feeds data-source query"))]
    Get {
        /// Database ID
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
enum DataSourceCmd {
    /// List data sources shared with the integration (a title search)
    #[command(after_long_help = outdoc::rest_list("raw Notion data source objects", &["id"], &outdoc::END_CURSOR))]
    List {
        /// Title text to match
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a data source and its property schema
    #[command(after_long_help = outdoc::rest_obj("raw Notion data source object", "id"))]
    Get {
        /// Data source ID
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Query a data source's rows
    #[command(after_long_help = outdoc::rest_list("raw Notion page objects (rows)", &["id"], &outdoc::END_CURSOR))]
    Query {
        /// Data source ID
        id: String,
        /// Notion filter object as JSON
        #[arg(long)]
        filter: Option<String>,
        /// Notion sorts array as JSON
        #[arg(long)]
        sorts: Option<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
}

#[derive(Subcommand)]
enum BlockCmd {
    /// List a page's or block's child blocks (one level)
    #[command(after_long_help = outdoc::rest_list("raw Notion block objects; has_children marks blocks to list next", &["id"], &outdoc::END_CURSOR))]
    List {
        /// Page or block ID
        id: String,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a block
    #[command(after_long_help = outdoc::rest_obj("raw Notion block object", "id"))]
    Get {
        /// Block ID
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Append child blocks to a page or block
    #[command(after_long_help = outdoc::rest_obj("raw Notion list object; results[] holds the appended blocks", "results[].id"))]
    Append {
        /// Page or block ID
        id: String,
        /// Notion block objects as a JSON array
        #[arg(
            long,
            conflicts_with = "children_file",
            required_unless_present = "children_file"
        )]
        children: Option<String>,
        /// Read the JSON array of blocks from a file
        #[arg(long, conflicts_with = "children")]
        children_file: Option<PathBuf>,
        /// Insert after this child block instead of at the end
        #[arg(long)]
        after: Option<String>,
    },
}

#[derive(Subcommand)]
enum CommentCmd {
    /// List open comments on a page or block
    #[command(after_long_help = outdoc::rest_list("raw Notion comment objects", &["id", "discussion_id"], &outdoc::END_CURSOR))]
    List {
        /// Page or block ID
        id: String,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Comment on a page or block, or reply in a discussion
    #[command(after_long_help = outdoc::rest_obj("raw Notion comment object", "id"))]
    Create {
        #[command(flatten)]
        target: CommentTarget,
        #[command(flatten)]
        body: BodyInput,
    },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct CommentTarget {
    /// Page ID: starts a new discussion on the page
    #[arg(long)]
    page: Option<String>,
    /// Block ID: starts a new discussion on the block
    #[arg(long)]
    block: Option<String>,
    /// Discussion ID: replies in an existing thread
    #[arg(long)]
    discussion: Option<String>,
}

#[derive(Subcommand)]
enum UserCmd {
    /// List workspace users
    #[command(after_long_help = outdoc::rest_list("raw Notion user objects", &["id"], &outdoc::END_CURSOR))]
    List {
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a user by ID, or `me` for the integration's bot user
    #[command(after_long_help = outdoc::rest_obj("raw Notion user object", "id"))]
    Get {
        /// User ID or `me`
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["v1".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let api = api(crate::auth::notion_token(instance)?, format)?;
    dispatch(&api, cmd.command)
}

fn dispatch(api: &Api, cmd: Resource) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Resource::Page(cmd) => run_page(api, cmd),
        Resource::Database(DatabaseCmd::Get { id, from }) => {
            pipe::run_get(id, from, api.format, |id| {
                api.get_body(path!["databases", id], Vec::new())
            })
        }
        Resource::DataSource(cmd) => run_data_source(api, cmd),
        Resource::Block(cmd) => run_block(api, cmd),
        Resource::Comment(cmd) => run_comment(api, cmd),
        Resource::Search { query, cursor } => search(api, query, None, cursor),
        Resource::User(UserCmd::List { cursor }) => {
            print_list(api, Method::GET, path!["users"], cursor, None)
        }
        Resource::User(UserCmd::Get { id, from }) => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["users", id], Vec::new())
        }),
    }
}

pub fn authenticated() -> bool {
    crate::auth::notion_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("notion")
}

pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    let url = reqwest::Url::parse(&format!("{BASE_URL}/v1/users/me"))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        &[("Notion-Version", VERSION)],
        &[reqwest::StatusCode::UNAUTHORIZED],
    )
}

fn run_page(api: &Api, cmd: PageCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PageCmd::List { query, cursor } => search(api, query, Some("page"), cursor),
        PageCmd::Get { id, markdown, from } => pipe::run_get(id, from, api.format, |id| {
            let segments = if markdown {
                path!["pages", id, "markdown"]
            } else {
                path!["pages", id]
            };
            api.get_body(segments, Vec::new())
        }),
        PageCmd::Create {
            parent,
            title,
            properties,
            body,
        } => {
            let mut payload = Map::new();
            let parent = match (parent.parent, parent.data_source) {
                (Some(id), None) => Some(json!({ "page_id": id })),
                (None, Some(id)) => Some(json!({ "data_source_id": id })),
                (None, None) => None,
                _ => unreachable!("clap enforces --parent xor --data-source"),
            };
            insert_opt(&mut payload, "parent", parent);
            let properties = page_properties(title, properties)?;
            if !properties.is_empty() {
                payload.insert("properties".into(), properties.into());
            }
            insert_opt(&mut payload, "markdown", body.read()?);
            api.print(
                Method::POST,
                path!["pages"],
                Vec::new(),
                Some(payload.into()),
            )
        }
        PageCmd::Update {
            id,
            title,
            properties,
            body,
        } => {
            let properties = page_properties(title, properties)?;
            let body = body.read()?;
            let mut response = None;
            if !properties.is_empty() {
                let payload = json!({ "properties": properties });
                response = Some(
                    api.send(Method::PATCH, &path!["pages", id], &[], Some(payload))?
                        .body,
                );
            }
            if let Some(body) = body {
                let payload = json!({
                    "type": "replace_content",
                    "replace_content": { "new_str": body },
                });
                response = Some(
                    api.send(
                        Method::PATCH,
                        &path!["pages", id, "markdown"],
                        &[],
                        Some(payload),
                    )?
                    .body,
                );
            }
            let response =
                response.ok_or("nothing to update: pass --title, --properties, or --body")?;
            crate::output::print(&response, api.format);
            Ok(())
        }
        PageCmd::Delete { id } => api.print(
            Method::PATCH,
            path!["pages", id],
            Vec::new(),
            Some(json!({ "in_trash": true })),
        ),
    }
}

/// `--properties` as given, with `--title` written to the `title` property
/// ID, which every page has whatever the title column is named.
fn page_properties(
    title: Option<String>,
    input: PropertiesInput,
) -> Result<Map<String, Value>, Box<dyn std::error::Error>> {
    let mut properties = match read_json(input.properties, input.properties_file, "--properties")? {
        Some(Value::Object(properties)) => properties,
        Some(_) => return Err("--properties must be a JSON object".into()),
        None => Map::new(),
    };
    if let Some(title) = title {
        properties.insert(
            "title".into(),
            json!({ "title": [{ "text": { "content": title } }] }),
        );
    }
    Ok(properties)
}

fn run_data_source(api: &Api, cmd: DataSourceCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DataSourceCmd::List { query, cursor } => search(api, query, Some("data_source"), cursor),
        DataSourceCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["data_sources", id], Vec::new())
        }),
        DataSourceCmd::Query {
            id,
            filter,
            sorts,
            cursor,
        } => {
            let mut payload = Map::new();
            insert_opt(
                &mut payload,
                "filter",
                rest::parse_json(filter, "--filter")?,
            );
            insert_opt(&mut payload, "sorts", rest::parse_json(sorts, "--sorts")?);
            print_list(
                api,
                Method::POST,
                path!["data_sources", id, "query"],
                cursor,
                Some(payload),
            )
        }
    }
}

fn run_block(api: &Api, cmd: BlockCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        BlockCmd::List { id, cursor } => print_list(
            api,
            Method::GET,
            path!["blocks", id, "children"],
            cursor,
            None,
        ),
        BlockCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["blocks", id], Vec::new())
        }),
        BlockCmd::Append {
            id,
            children,
            children_file,
            after,
        } => {
            let children = read_json(children, children_file, "--children")?
                .filter(Value::is_array)
                .ok_or("--children must be a JSON array of block objects")?;
            let mut payload = Map::new();
            payload.insert("children".into(), children);
            insert_opt(
                &mut payload,
                "position",
                after.map(|id| json!({ "type": "after_block", "after_block": { "id": id } })),
            );
            api.print(
                Method::PATCH,
                path!["blocks", id, "children"],
                Vec::new(),
                Some(payload.into()),
            )
        }
    }
}

fn run_comment(api: &Api, cmd: CommentCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommentCmd::List { id, cursor } => {
            let mut query = vec![("block_id", id)];
            query.extend(cursor_query(cursor));
            let response = api.send(Method::GET, &path!["comments"], &query, None)?;
            print_wrapped(api, response.body)
        }
        CommentCmd::Create { target, body } => {
            let mut payload = match (target.page, target.block, target.discussion) {
                (Some(id), None, None) => json!({ "parent": { "page_id": id } }),
                (None, Some(id), None) => json!({ "parent": { "block_id": id } }),
                (None, None, Some(id)) => json!({ "discussion_id": id }),
                _ => unreachable!("clap enforces one of --page, --block, --discussion"),
            };
            payload["markdown"] = body.required()?.into();
            api.print(Method::POST, path!["comments"], Vec::new(), Some(payload))
        }
    }
}

/// Title search; `object` narrows it to `page` or `data_source`.
fn search(
    api: &Api,
    query: Option<String>,
    object: Option<&str>,
    cursor: Cursor,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut payload = Map::new();
    insert_opt(&mut payload, "query", query);
    insert_opt(
        &mut payload,
        "filter",
        object.map(|value| json!({ "property": "object", "value": value })),
    );
    print_list(api, Method::POST, path!["search"], cursor, Some(payload))
}

fn api(token: String, format: crate::output::Format) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(BASE_URL)?,
        auth: Auth::Bearer(token),
        format,
        headers: vec![("Notion-Version", VERSION.to_owned())],
        trailing_slash: false,
    })
}

fn cursor_query(cursor: Cursor) -> Vec<(&'static str, String)> {
    let mut query = vec![("page_size", cursor.limit.to_string())];
    rest::push_query(&mut query, "start_cursor", cursor.after);
    query
}

/// GET lists page through the query string, POST lists (search, query)
/// through the JSON body.
fn print_list(
    api: &Api,
    method: Method,
    segments: Vec<String>,
    cursor: Cursor,
    body: Option<Map<String, Value>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = match body {
        Some(mut body) => {
            body.insert("page_size".into(), cursor.limit.into());
            insert_opt(&mut body, "start_cursor", cursor.after);
            api.send(method, &segments, &[], Some(body.into()))?
        }
        None => api.send(method, &segments, &cursor_query(cursor), None)?,
    };
    print_wrapped(api, response.body)
}

fn print_wrapped(api: &Api, body: Value) -> Result<(), Box<dyn std::error::Error>> {
    crate::output::print(&wrap(body)?, api.format);
    Ok(())
}

/// Notion lists are `{"object": "list", "results", "has_more", "next_cursor"}`.
fn wrap(mut body: Value) -> Result<Value, Box<dyn std::error::Error>> {
    let items = body["results"]
        .as_array_mut()
        .map(std::mem::take)
        .ok_or("Notion list response did not contain results")?;
    let cursor = body["next_cursor"]
        .as_str()
        .filter(|_| body["has_more"] == true);
    Ok(rest::wrap_list(
        items,
        json!({ "hasNextPage": cursor.is_some(), "endCursor": cursor }),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn cursor(after: Option<&str>) -> Cursor {
        Cursor {
            limit: 10,
            after: after.map(str::to_owned),
        }
    }

    fn request_body(request: &str) -> Value {
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
    }

    #[test]
    fn wrap_reads_the_cursor_only_while_has_more() {
        let more = wrap(json!({"results": [1], "has_more": true, "next_cursor": "c2"})).unwrap();
        assert_eq!(
            more,
            json!({"items": [1], "pageInfo": {"hasNextPage": true, "endCursor": "c2"}})
        );
        let last = wrap(json!({"results": [], "has_more": false, "next_cursor": null})).unwrap();
        assert_eq!(
            last["pageInfo"],
            json!({"hasNextPage": false, "endCursor": null})
        );
        assert!(wrap(json!({"object": "error"})).is_err());
    }

    #[test]
    fn requests_carry_bearer_auth_and_the_notion_version() {
        let (api, request_rx, server) = test_api("{\"results\":[],\"has_more\":false}");
        dispatch(
            &api,
            Resource::Block(BlockCmd::List {
                id: "page-1".into(),
                cursor: cursor(Some("abc")),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(
            request.starts_with("get /v1/blocks/page-1/children?page_size=10&start_cursor=abc ")
        );
        assert!(request.contains("authorization: bearer ntn-secret"));
        assert!(request.contains(&format!("notion-version: {VERSION}")));
    }

    #[test]
    fn page_list_searches_pages_with_the_cursor_in_the_body() {
        let (api, request_rx, server) = test_api("{\"results\":[],\"has_more\":false}");
        dispatch(
            &api,
            Resource::Page(PageCmd::List {
                query: Some("spec".into()),
                cursor: cursor(Some("abc")),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v1/search "));
        assert_eq!(
            request_body(&request),
            json!({
                "query": "spec",
                "filter": {"property": "object", "value": "page"},
                "page_size": 10,
                "start_cursor": "abc",
            })
        );
    }

    #[test]
    fn page_create_sends_parent_title_properties_and_markdown() {
        let (api, request_rx, server) = test_api("{}");
        dispatch(
            &api,
            Resource::Page(PageCmd::Create {
                parent: PageParent {
                    parent: None,
                    data_source: Some("ds-1".into()),
                },
                title: Some("Spec".into()),
                properties: PropertiesInput {
                    properties: Some(r#"{"Status": {"status": {"name": "Draft"}}}"#.into()),
                    properties_file: None,
                },
                body: BodyInput {
                    body: Some("# Goals".into()),
                    body_file: None,
                },
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v1/pages "));
        assert_eq!(
            request_body(&request),
            json!({
                "parent": {"data_source_id": "ds-1"},
                "properties": {
                    "Status": {"status": {"name": "Draft"}},
                    "title": {"title": [{"text": {"content": "Spec"}}]},
                },
                "markdown": "# Goals",
            })
        );
    }

    #[test]
    fn page_update_body_replaces_content_through_the_markdown_endpoint() {
        let (api, request_rx, server) = test_api("{}");
        dispatch(
            &api,
            Resource::Page(PageCmd::Update {
                id: "page-1".into(),
                title: None,
                properties: PropertiesInput {
                    properties: None,
                    properties_file: None,
                },
                body: BodyInput {
                    body: Some("new".into()),
                    body_file: None,
                },
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("PATCH /v1/pages/page-1/markdown "));
        assert_eq!(
            request_body(&request),
            json!({"type": "replace_content", "replace_content": {"new_str": "new"}})
        );
    }

    #[test]
    fn page_update_without_fields_is_an_error() {
        let api = test_api_at(reqwest::Url::parse("http://127.0.0.1:9").unwrap());
        let error = run_page(
            &api,
            PageCmd::Update {
                id: "page-1".into(),
                title: None,
                properties: PropertiesInput {
                    properties: None,
                    properties_file: None,
                },
                body: BodyInput::default(),
            },
        )
        .unwrap_err();
        assert!(error.to_string().starts_with("nothing to update"));
    }

    #[test]
    fn block_append_sends_children_and_an_after_position() {
        let (api, request_rx, server) = test_api("{}");
        dispatch(
            &api,
            Resource::Block(BlockCmd::Append {
                id: "page-1".into(),
                children: Some(r#"[{"paragraph": {"rich_text": []}}]"#.into()),
                children_file: None,
                after: Some("block-9".into()),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("PATCH /v1/blocks/page-1/children "));
        assert_eq!(
            request_body(&request),
            json!({
                "children": [{"paragraph": {"rich_text": []}}],
                "position": {"type": "after_block", "after_block": {"id": "block-9"}},
            })
        );
    }

    #[test]
    fn comment_create_replies_in_a_discussion_with_markdown() {
        let (api, request_rx, server) = test_api("{}");
        dispatch(
            &api,
            Resource::Comment(CommentCmd::Create {
                target: CommentTarget {
                    page: None,
                    block: None,
                    discussion: Some("d-1".into()),
                },
                body: BodyInput {
                    body: Some("LGTM".into()),
                    body_file: None,
                },
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v1/comments "));
        assert_eq!(
            request_body(&request),
            json!({"discussion_id": "d-1", "markdown": "LGTM"})
        );
    }

    #[test]
    fn data_source_query_posts_filter_and_sorts() {
        let (api, request_rx, server) = test_api("{\"results\":[],\"has_more\":false}");
        dispatch(
            &api,
            Resource::DataSource(DataSourceCmd::Query {
                id: "ds-1".into(),
                filter: Some(r#"{"property": "Done", "checkbox": {"equals": false}}"#.into()),
                sorts: None,
                cursor: cursor(None),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v1/data_sources/ds-1/query "));
        assert_eq!(
            request_body(&request),
            json!({
                "filter": {"property": "Done", "checkbox": {"equals": false}},
                "page_size": 10,
            })
        );
    }

    fn test_api_at(url: reqwest::Url) -> Api {
        Api {
            base_url: url,
            ..api("ntn-secret".into(), crate::output::Format::Json).unwrap()
        }
    }

    fn test_api(body: &str) -> (Api, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let (url, request_rx, server) = rest::testing::test_server("200 OK", body, "");
        (test_api_at(url), request_rx, server)
    }
}
