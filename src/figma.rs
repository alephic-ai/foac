use std::path::PathBuf;

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::rest::{self, Api, Auth, insert_opt, push_query};

const API_URL: &str = "https://api.figma.com";

#[derive(Args)]
pub struct Cmd {
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Projects in a team
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Files: document trees, nodes, and version history
    #[command(subcommand)]
    File(FileCmd),
    /// Comments on a file
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Rendered image URLs for nodes
    #[command(subcommand)]
    Image(ImageCmd),
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// List a team's projects; the team ID is the number in the team page URL
    List { team_id: String },
}

#[derive(Subcommand)]
enum FileCmd {
    /// List the files in a project
    List { project_id: String },
    /// Get a file's document tree (can be huge; scope it with --depth or --ids)
    Get {
        /// File key, or a pasted figma.com file URL
        file: String,
        /// Comma-separated node IDs to restrict the document to
        #[arg(long)]
        ids: Option<String>,
        /// Levels of the document tree to return
        #[arg(long)]
        depth: Option<u32>,
        /// Version ID; defaults to the latest
        #[arg(long)]
        version: Option<String>,
        /// Set to "paths" to include vector data
        #[arg(long)]
        geometry: Option<String>,
    },
    /// Get specific nodes and their subtrees
    Nodes {
        /// File key, or a pasted figma.com file URL
        file: String,
        /// Comma-separated node IDs like 1:2 (a URL's node-id=1-2 also works)
        #[arg(long)]
        ids: String,
        /// Levels below each node to return
        #[arg(long)]
        depth: Option<u32>,
        /// Version ID; defaults to the latest
        #[arg(long)]
        version: Option<String>,
        /// Set to "paths" to include vector data
        #[arg(long)]
        geometry: Option<String>,
    },
    /// List a file's version history
    Versions {
        /// File key, or a pasted figma.com file URL
        file: String,
        /// Version ID to page backwards from (pageInfo.nextBefore)
        #[arg(long)]
        before: Option<u64>,
    },
}

#[derive(Subcommand)]
enum CommentCmd {
    /// List a file's comments
    List {
        /// File key, or a pasted figma.com file URL
        file: String,
        /// Return comment bodies as markdown
        #[arg(long)]
        as_md: bool,
    },
    /// Post a comment or a reply
    Create {
        /// File key, or a pasted figma.com file URL
        file: String,
        #[command(flatten)]
        body: BodyInput,
        /// Comment ID to reply to
        #[arg(long)]
        reply_to: Option<String>,
    },
    /// Delete a comment
    Delete {
        /// File key, or a pasted figma.com file URL
        file: String,
        comment_id: String,
    },
}

#[derive(Subcommand)]
enum ImageCmd {
    /// Get rendered image URLs for nodes; the response maps node IDs to URLs
    Export {
        /// File key, or a pasted figma.com file URL
        file: String,
        /// Comma-separated node IDs like 1:2 (a URL's node-id=1-2 also works)
        #[arg(long)]
        ids: String,
        /// png, jpg, svg, or pdf
        #[arg(long)]
        image_format: Option<String>,
        /// Render scale between 0.01 and 4
        #[arg(long)]
        scale: Option<f64>,
        /// Version ID; defaults to the latest
        #[arg(long)]
        version: Option<String>,
    },
}

#[derive(Args)]
struct BodyInput {
    /// Comment text
    #[arg(long, conflicts_with = "body_file")]
    body: Option<String>,
    /// Read comment text from a UTF-8 file
    #[arg(long, conflicts_with = "body")]
    body_file: Option<PathBuf>,
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["v1".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(cmd: Cmd, format: crate::output::Format) -> Result<(), Box<dyn std::error::Error>> {
    let api = api(crate::auth::figma_token()?, format)?;
    match cmd.command {
        Resource::Project(cmd) => run_project(&api, cmd),
        Resource::File(cmd) => run_file(&api, cmd),
        Resource::Comment(cmd) => run_comment(&api, cmd),
        Resource::Image(cmd) => run_image(&api, cmd),
    }
}

pub fn authenticated() -> bool {
    crate::auth::figma_token().is_ok()
}

pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    auth_identity_at(
        token,
        reqwest::Url::parse(&format!("{API_URL}/v1/me")).unwrap(),
    )
}

fn auth_identity_at(token: &str, url: reqwest::Url) -> Result<Value, crate::auth::ValidationError> {
    // Figma rejects bad tokens with 403, not only 401.
    rest::identity(
        url,
        &Auth::Header("X-Figma-Token", token.to_owned()),
        &[],
        &[
            reqwest::StatusCode::UNAUTHORIZED,
            reqwest::StatusCode::FORBIDDEN,
        ],
    )
}

fn run_project(api: &Api, cmd: ProjectCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ProjectCmd::List { team_id } => print_list(
            api,
            path!["teams", team_id, "projects"],
            Vec::new(),
            "projects",
        ),
    }
}

fn run_file(api: &Api, cmd: FileCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        FileCmd::List { project_id } => print_list(
            api,
            path!["projects", project_id, "files"],
            Vec::new(),
            "files",
        ),
        FileCmd::Get {
            file,
            ids,
            depth,
            version,
            geometry,
        } => {
            let mut query = Vec::new();
            push_query(&mut query, "ids", ids.as_deref().map(node_ids));
            push_query(&mut query, "depth", depth);
            push_query(&mut query, "version", version);
            push_query(&mut query, "geometry", geometry);
            api.print(Method::GET, path!["files", file_key(&file)], query, None)
        }
        FileCmd::Nodes {
            file,
            ids,
            depth,
            version,
            geometry,
        } => {
            let mut query = vec![("ids", node_ids(&ids))];
            push_query(&mut query, "depth", depth);
            push_query(&mut query, "version", version);
            push_query(&mut query, "geometry", geometry);
            api.print(
                Method::GET,
                path!["files", file_key(&file), "nodes"],
                query,
                None,
            )
        }
        FileCmd::Versions { file, before } => {
            let mut query = Vec::new();
            push_query(&mut query, "before", before);
            print_list(
                api,
                path!["files", file_key(&file), "versions"],
                query,
                "versions",
            )
        }
    }
}

fn run_comment(api: &Api, cmd: CommentCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommentCmd::List { file, as_md } => {
            let mut query = Vec::new();
            if as_md {
                query.push(("as_md", "true".into()));
            }
            print_list(
                api,
                path!["files", file_key(&file), "comments"],
                query,
                "comments",
            )
        }
        CommentCmd::Create {
            file,
            body,
            reply_to,
        } => {
            let mut payload = Map::new();
            payload.insert("message".into(), body.required()?.into());
            insert_opt(&mut payload, "comment_id", reply_to);
            api.print(
                Method::POST,
                path!["files", file_key(&file), "comments"],
                Vec::new(),
                Some(payload.into()),
            )
        }
        CommentCmd::Delete { file, comment_id } => api.print(
            Method::DELETE,
            path!["files", file_key(&file), "comments", comment_id],
            Vec::new(),
            None,
        ),
    }
}

fn run_image(api: &Api, cmd: ImageCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ImageCmd::Export {
            file,
            ids,
            image_format,
            scale,
            version,
        } => {
            let mut query = vec![("ids", node_ids(&ids))];
            push_query(&mut query, "format", image_format);
            push_query(&mut query, "scale", scale);
            push_query(&mut query, "version", version);
            api.print(Method::GET, path!["images", file_key(&file)], query, None)
        }
    }
}

fn api(token: String, format: crate::output::Format) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(API_URL)?,
        auth: Auth::Header("X-Figma-Token", token),
        format,
        headers: &[],
        trailing_slash: false,
    })
}

fn print_list(
    api: &Api,
    segments: Vec<String>,
    query: Vec<(&'static str, String)>,
    key: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = api.send(Method::GET, &segments, &query, None)?;
    let items = response.body[key]
        .as_array()
        .cloned()
        .ok_or_else(|| format!("Figma list response did not contain an array at {key}"))?;
    crate::output::print(
        &rest::wrap_list(items, page_info(&response.body)),
        api.format,
    );
    Ok(())
}

impl BodyInput {
    fn required(self) -> Result<String, Box<dyn std::error::Error>> {
        match (self.body, self.body_file) {
            (Some(body), None) => Ok(body),
            (None, Some(path)) => Ok(std::fs::read_to_string(path)?),
            (None, None) => Err("one of --body or --body-file is required".into()),
            (Some(_), Some(_)) => unreachable!("clap enforces --body xor --body-file"),
        }
    }
}

/// Accept a raw file key or a pasted figma.com URL like
/// `https://www.figma.com/design/KEY/Name?node-id=1-2` and return the key.
fn file_key(input: &str) -> String {
    let Some((_, path)) = input.split_once("figma.com/") else {
        return input.to_owned();
    };
    let mut segments = path.split('/');
    match (segments.next(), segments.next()) {
        (Some("file" | "design" | "board" | "proto" | "slides"), Some(key)) if !key.is_empty() => {
            key.split(['?', '#']).next().unwrap_or(key).to_owned()
        }
        _ => input.to_owned(),
    }
}

/// The API wants node IDs like `1:2`; Figma URLs write them as `node-id=1-2`.
/// Convert dashed IDs and leave canonical ones untouched.
fn node_ids(ids: &str) -> String {
    ids.split(',')
        .map(|id| {
            let id = id.trim();
            if id.contains(':') {
                id.to_owned()
            } else {
                id.replace('-', ":")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Only `file versions` paginates: its `pagination.next_page` is a URL whose
/// `before` parameter is the cursor for the next request. Everything else
/// returns no pagination object and reports `hasNextPage: false`.
fn page_info(body: &Value) -> Value {
    let next = next_before(body);
    json!({
        "hasNextPage": next.is_some(),
        "nextBefore": next,
    })
}

fn next_before(body: &Value) -> Option<String> {
    let next = body.pointer("/pagination/next_page")?.as_str()?;
    reqwest::Url::parse(next)
        .ok()?
        .query_pairs()
        .find(|(key, _)| key == "before")
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_file_keys_from_pasted_urls() {
        assert_eq!(file_key("AbC123"), "AbC123");
        for url in [
            "https://www.figma.com/design/AbC123/My-File?node-id=1-2",
            "https://www.figma.com/file/AbC123/My-File",
            "https://figma.com/board/AbC123",
            "https://www.figma.com/proto/AbC123?page-id=0",
            "https://www.figma.com/slides/AbC123#anchor",
        ] {
            assert_eq!(file_key(url), "AbC123");
        }
        assert_eq!(
            file_key("https://www.figma.com/files/recent"),
            "https://www.figma.com/files/recent"
        );
    }

    #[test]
    fn normalizes_url_style_node_ids() {
        assert_eq!(node_ids("1:2"), "1:2");
        assert_eq!(node_ids("1-2"), "1:2");
        assert_eq!(node_ids("1-2, 3:4"), "1:2,3:4");
        assert_eq!(node_ids("I1:2;3:4"), "I1:2;3:4");
    }

    #[test]
    fn reads_version_cursors_from_pagination_urls() {
        let body = serde_json::json!({
            "versions": [],
            "pagination": {
                "next_page": "https://api.figma.com/v1/files/AbC123/versions?before=42",
            },
        });
        let info = page_info(&body);
        assert_eq!(info["hasNextPage"], true);
        assert_eq!(info["nextBefore"], "42");
        assert_eq!(page_info(&serde_json::json!({}))["hasNextPage"], false);
    }

    #[test]
    fn validates_identity_with_the_figma_token_header() {
        let (url, request_rx, server) = rest::testing::test_server(
            "200 OK",
            r#"{"id":"1","email":"person@example.com","handle":"person"}"#,
            "",
        );
        let identity = auth_identity_at("figma-token", url.join("v1/me").unwrap()).unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /v1/me http/1.1"));
        assert!(request.contains("x-figma-token: figma-token"));
        assert!(!request.contains("authorization:"));
        assert_eq!(identity["handle"], "person");

        let (url, _, server) =
            rest::testing::test_server("403 Forbidden", r#"{"err":"Invalid token"}"#, "");
        let error = auth_identity_at("bad", url.join("v1/me").unwrap()).unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Rejected(_)));
    }

    #[test]
    fn comment_create_posts_message_and_reply_target() {
        let (url, request_rx, server) = rest::testing::test_server("200 OK", "{}", "");
        let api = Api {
            client: reqwest::blocking::Client::new(),
            base_url: url,
            auth: Auth::Header("X-Figma-Token", "figma-token".into()),
            format: crate::output::Format::Json,
            headers: &[],
            trailing_slash: false,
        };
        run_comment(
            &api,
            CommentCmd::Create {
                file: "https://www.figma.com/design/AbC123/My-File".into(),
                body: BodyInput {
                    body: Some("Looks good".into()),
                    body_file: None,
                },
                reply_to: Some("123".into()),
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v1/files/AbC123/comments HTTP/1.1"));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            serde_json::json!({ "message": "Looks good", "comment_id": "123" })
        );
    }
}
