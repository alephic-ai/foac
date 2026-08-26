use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::rest::{self, insert_opt, parse_response};

const API_URL: &str = "https://slack.com/api/";

#[derive(Args)]
pub struct Cmd {
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Workspace conversations (channels and direct messages)
    #[command(subcommand)]
    Conversation(ConversationCmd),
    /// Messages and thread replies
    #[command(subcommand)]
    Message(MessageCmd),
    /// Workspace users
    #[command(subcommand)]
    User(UserCmd),
    /// Search workspace messages (requires a Slack user credential)
    Search {
        /// Slack search query, e.g. `"deployment in:eng from:@alice"`
        query: String,
        /// Sort matches by score or timestamp
        #[arg(long, value_enum)]
        sort: Option<SearchSort>,
        /// Sort direction
        #[arg(long, value_enum)]
        direction: Option<Direction>,
        /// Include Slack's match-highlight markers
        #[arg(long)]
        highlight: bool,
        #[command(flatten)]
        page: SearchPage,
    },
    /// Emoji reactions on messages
    #[command(subcommand)]
    Reaction(ReactionCmd),
}

#[derive(Args, Clone)]
struct Page {
    /// Results per page
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=200))]
    limit: u16,
    /// Opaque cursor from pageInfo.endCursor
    #[arg(long)]
    after: Option<String>,
}

#[derive(Args, Clone)]
struct SearchPage {
    /// Results per page
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u8).range(1..=100))]
    limit: u8,
    /// Opaque cursor from pageInfo.endCursor
    #[arg(long)]
    after: Option<String>,
}

#[derive(Args, Default)]
struct BodyInput {
    /// Message text (Slack mrkdwn is supported)
    #[arg(long, conflicts_with = "body_file")]
    body: Option<String>,
    /// Read message text from a UTF-8 file
    #[arg(long, conflicts_with = "body")]
    body_file: Option<PathBuf>,
}

#[derive(Clone, Copy, ValueEnum)]
enum SearchSort {
    Score,
    Timestamp,
}

impl SearchSort {
    fn as_str(self) -> &'static str {
        match self {
            Self::Score => "score",
            Self::Timestamp => "timestamp",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Direction {
    Asc,
    Desc,
}

impl Direction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

#[derive(Subcommand)]
enum ConversationCmd {
    /// List conversations visible to the token
    List {
        /// Comma-separated Slack conversation types
        #[arg(long, default_value = "public_channel,private_channel,mpim,im")]
        types: String,
        /// Exclude archived conversations
        #[arg(long)]
        exclude_archived: bool,
        #[command(flatten)]
        page: Page,
    },
    /// Get a conversation by ID or channel name such as #eng
    Get { conversation: String },
}

#[derive(Subcommand)]
enum MessageCmd {
    /// List channel history or replies in a thread
    List {
        /// Conversation ID or channel name such as #eng
        conversation: String,
        /// Parent message timestamp; lists replies when supplied
        #[arg(long)]
        thread_ts: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get one message by timestamp
    Get {
        /// Conversation ID or channel name such as #eng
        conversation: String,
        /// Message timestamp
        ts: String,
        /// Parent message timestamp when getting a thread reply
        #[arg(long)]
        thread_ts: Option<String>,
    },
    /// Post a message or thread reply
    Create {
        /// Conversation ID or channel name such as #eng
        conversation: String,
        #[command(flatten)]
        body: BodyInput,
        /// Parent message timestamp; creates a reply when supplied
        #[arg(long)]
        thread_ts: Option<String>,
        /// Also post the reply to the conversation
        #[arg(long, requires = "thread_ts")]
        reply_broadcast: bool,
    },
    /// Update a message posted by the authenticated user
    Update {
        /// Conversation ID or channel name such as #eng
        conversation: String,
        /// Message timestamp
        ts: String,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Delete a message posted by the authenticated user
    Delete {
        /// Conversation ID or channel name such as #eng
        conversation: String,
        /// Message timestamp
        ts: String,
    },
}

#[derive(Subcommand)]
enum UserCmd {
    /// List workspace users
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get a user by ID, @name, display name, or email
    Get { user: String },
}

#[derive(Subcommand)]
enum ReactionCmd {
    /// Add an emoji reaction to a message
    Add {
        /// Conversation ID or channel name such as #eng
        conversation: String,
        /// Message timestamp
        ts: String,
        /// Emoji name without colons
        name: String,
    },
    /// Remove an emoji reaction from a message
    Remove {
        /// Conversation ID or channel name such as #eng
        conversation: String,
        /// Message timestamp
        ts: String,
        /// Emoji name without colons
        name: String,
    },
}

struct Api {
    client: reqwest::blocking::Client,
    base_url: reqwest::Url,
    token: String,
    format: crate::output::Format,
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd.command {
        Resource::Search {
            query,
            sort,
            direction,
            highlight,
            page,
        } => {
            let api = Api::slack(crate::auth::slack_user_token(instance)?, format)?;
            run_search(&api, query, sort, direction, highlight, page)
        }
        command => {
            let api = Api::slack(crate::auth::slack_token(instance)?, format)?;
            match command {
                Resource::Conversation(cmd) => run_conversation(&api, cmd),
                Resource::Message(cmd) => run_message(&api, cmd),
                Resource::User(cmd) => run_user(&api, cmd),
                Resource::Reaction(cmd) => run_reaction(&api, cmd),
                Resource::Search { .. } => unreachable!(),
            }
        }
    }
}

pub fn authenticated() -> bool {
    crate::auth::slack_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("slack")
}

pub(crate) fn is_bot_token(token: &str) -> bool {
    token.starts_with("xoxb-") || token.starts_with("xoxe.xoxb-")
}

pub(crate) fn is_user_token(token: &str) -> bool {
    token.starts_with("xoxp-") || token.starts_with("xoxe.xoxp-")
}

pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    auth_identity_at(
        token,
        reqwest::Url::parse(&format!("{API_URL}auth.test")).unwrap(),
    )
}

fn auth_identity_at(token: &str, url: reqwest::Url) -> Result<Value, crate::auth::ValidationError> {
    let (status, body) = rest::fetch_identity(
        Method::POST,
        url,
        &rest::Auth::Bearer(token.to_owned()),
        &[],
    )?;
    if !status.is_success() {
        return Err(crate::auth::ValidationError::Failed(body.to_string()));
    }
    if body["ok"] != true {
        return Err(crate::auth::ValidationError::Rejected(body.to_string()));
    }
    Ok(body)
}

fn run_conversation(api: &Api, cmd: ConversationCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ConversationCmd::List {
            types,
            exclude_archived,
            page,
        } => {
            let mut query = page_query(page);
            query.push(("types", types));
            if exclude_archived {
                query.push(("exclude_archived", "true".into()));
            }
            api.print_list("conversations.list", query, ListShape::Key("channels"))
        }
        ConversationCmd::Get { conversation } => {
            let channel = resolve_conversation(api, &conversation)?;
            api.print(
                Method::GET,
                "conversations.info",
                vec![("channel", channel)],
                None,
            )
        }
    }
}

fn run_message(api: &Api, cmd: MessageCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        MessageCmd::List {
            conversation,
            thread_ts,
            page,
        } => {
            let channel = resolve_conversation(api, &conversation)?;
            let mut query = page_query(page);
            query.push(("channel", channel));
            let method = if let Some(thread_ts) = thread_ts {
                query.push(("ts", thread_ts));
                "conversations.replies"
            } else {
                "conversations.history"
            };
            api.print_list(method, query, ListShape::Key("messages"))
        }
        MessageCmd::Get {
            conversation,
            ts,
            thread_ts,
        } => {
            let channel = resolve_conversation(api, &conversation)?;
            let mut query = vec![("channel", channel), ("limit", "1".into())];
            let method = if let Some(thread_ts) = thread_ts {
                query.push(("ts", thread_ts));
                query.push(("oldest", ts.clone()));
                query.push(("latest", ts));
                query.push(("inclusive", "true".into()));
                "conversations.replies"
            } else {
                query.push(("oldest", ts.clone()));
                query.push(("latest", ts));
                query.push(("inclusive", "true".into()));
                "conversations.history"
            };
            api.print(Method::GET, method, query, None)
        }
        MessageCmd::Create {
            conversation,
            body,
            thread_ts,
            reply_broadcast,
        } => {
            let channel = resolve_conversation(api, &conversation)?;
            let mut payload = Map::new();
            payload.insert("channel".into(), channel.into());
            payload.insert("text".into(), body.required()?.into());
            insert_opt(&mut payload, "thread_ts", thread_ts);
            if reply_broadcast {
                payload.insert("reply_broadcast".into(), true.into());
            }
            api.print(
                Method::POST,
                "chat.postMessage",
                Vec::new(),
                Some(payload.into()),
            )
        }
        MessageCmd::Update {
            conversation,
            ts,
            body,
        } => {
            let channel = resolve_conversation(api, &conversation)?;
            api.print(
                Method::POST,
                "chat.update",
                Vec::new(),
                Some(json!({ "channel": channel, "ts": ts, "text": body.required()? })),
            )
        }
        MessageCmd::Delete { conversation, ts } => {
            let channel = resolve_conversation(api, &conversation)?;
            api.print(
                Method::POST,
                "chat.delete",
                Vec::new(),
                Some(json!({ "channel": channel, "ts": ts })),
            )
        }
    }
}

fn run_user(api: &Api, cmd: UserCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        UserCmd::List { page } => {
            api.print_list("users.list", page_query(page), ListShape::Key("members"))
        }
        UserCmd::Get { user } => {
            if is_user_id(&user) {
                return api.print(Method::GET, "users.info", vec![("user", user)], None);
            }
            if is_email(&user) {
                return api.print(
                    Method::GET,
                    "users.lookupByEmail",
                    vec![("email", user)],
                    None,
                );
            }
            let user = resolve_user(api, &user)?;
            api.print(Method::GET, "users.info", vec![("user", user)], None)
        }
    }
}

fn run_search(
    api: &Api,
    query_text: String,
    sort: Option<SearchSort>,
    direction: Option<Direction>,
    highlight: bool,
    page: SearchPage,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut query = vec![
        ("query", query_text),
        ("count", page.limit.to_string()),
        ("cursor", page.after.unwrap_or_else(|| "*".into())),
    ];
    if let Some(sort) = sort {
        query.push(("sort", sort.as_str().into()));
    }
    if let Some(direction) = direction {
        query.push(("sort_dir", direction.as_str().into()));
    }
    if highlight {
        query.push(("highlight", "true".into()));
    }
    api.print_list(
        "search.messages",
        query,
        ListShape::Nested("messages", "matches"),
    )
}

fn run_reaction(api: &Api, cmd: ReactionCmd) -> Result<(), Box<dyn std::error::Error>> {
    let (method, conversation, ts, name) = match cmd {
        ReactionCmd::Add {
            conversation,
            ts,
            name,
        } => ("reactions.add", conversation, ts, name),
        ReactionCmd::Remove {
            conversation,
            ts,
            name,
        } => ("reactions.remove", conversation, ts, name),
    };
    let channel = resolve_conversation(api, &conversation)?;
    api.print(
        Method::POST,
        method,
        Vec::new(),
        Some(json!({ "channel": channel, "timestamp": ts, "name": trim_colons(&name) })),
    )
}

#[derive(Clone, Copy)]
enum ListShape {
    Key(&'static str),
    Nested(&'static str, &'static str),
}

impl Api {
    fn slack(
        token: String,
        format: crate::output::Format,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            client: reqwest::blocking::Client::new(),
            base_url: reqwest::Url::parse(API_URL)?,
            token,
            format,
        })
    }

    fn print(
        &self,
        method: Method,
        api_method: &str,
        query: Vec<(&'static str, String)>,
        body: Option<Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = self.send(method, api_method, &query, body)?;
        crate::output::print(&body, self.format);
        Ok(())
    }

    fn print_list(
        &self,
        api_method: &str,
        query: Vec<(&'static str, String)>,
        shape: ListShape,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = self.send(Method::GET, api_method, &query, None)?;
        crate::output::print(&list_output(body, shape)?, self.format);
        Ok(())
    }

    fn send(
        &self,
        method: Method,
        api_method: &str,
        query: &[(&'static str, String)],
        body: Option<Value>,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let url = self.base_url.join(api_method)?;
        let mut request = self
            .client
            .request(method, url)
            .bearer_auth(&self.token)
            .header("User-Agent", "foac")
            .query(query);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send()?;
        let status = response.status();
        let body = parse_response(status, response.text()?);
        if !status.is_success() || body["ok"] == false {
            return Err(body.to_string().into());
        }
        Ok(body)
    }
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

fn resolve_conversation(api: &Api, value: &str) -> Result<String, Box<dyn std::error::Error>> {
    if is_conversation_id(value) {
        return Ok(value.to_owned());
    }
    let name = value.strip_prefix('#').unwrap_or(value);
    resolve_from_pages(api, "conversations.list", "channels", |item| {
        (item["name"].as_str() == Some(name))
            .then(|| item["id"].as_str().map(str::to_owned))
            .flatten()
    })?
    .ok_or_else(|| format!("could not resolve Slack conversation {value}").into())
}

fn resolve_user(api: &Api, value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let name = value.strip_prefix('@').unwrap_or(value);
    resolve_from_pages(api, "users.list", "members", |item| {
        let profile = &item["profile"];
        [
            item["name"].as_str(),
            profile["display_name"].as_str(),
            profile["real_name"].as_str(),
        ]
        .into_iter()
        .flatten()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
        .then(|| item["id"].as_str().map(str::to_owned))
        .flatten()
    })?
    .ok_or_else(|| format!("could not resolve Slack user {value}").into())
}

fn resolve_from_pages<F>(
    api: &Api,
    method: &str,
    key: &str,
    mut find: F,
) -> Result<Option<String>, Box<dyn std::error::Error>>
where
    F: FnMut(&Value) -> Option<String>,
{
    let mut cursor = None;
    loop {
        let mut query = vec![("limit", "200".into())];
        if method == "conversations.list" {
            query.push(("types", "public_channel,private_channel,mpim,im".into()));
        }
        if let Some(cursor) = cursor {
            query.push(("cursor", cursor));
        }
        let body = api.send(Method::GET, method, &query, None)?;
        let items = body[key]
            .as_array()
            .ok_or_else(|| format!("Slack {method} response did not contain an array at {key}"))?;
        if let Some(found) = items.iter().find_map(&mut find) {
            return Ok(Some(found));
        }
        cursor = next_cursor(&body);
        if cursor.is_none() {
            return Ok(None);
        }
    }
}

fn list_output(mut body: Value, shape: ListShape) -> Result<Value, Box<dyn std::error::Error>> {
    let cursor = next_cursor(&body);
    let items = match shape {
        ListShape::Key(key) => body
            .as_object_mut()
            .and_then(|object| object.remove(key))
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| format!("Slack list response did not contain an array at {key}"))?,
        ListShape::Nested(parent, key) => body
            .get_mut(parent)
            .and_then(Value::as_object_mut)
            .and_then(|object| object.remove(key))
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| {
                format!("Slack list response did not contain an array at {parent}.{key}")
            })?,
    };
    Ok(json!({
        "items": items,
        "pageInfo": {
            "hasNextPage": cursor.is_some(),
            "endCursor": cursor,
        },
    }))
}

fn next_cursor(body: &Value) -> Option<String> {
    body.pointer("/response_metadata/next_cursor")
        .or_else(|| body.pointer("/messages/pagination/next_cursor"))
        .and_then(Value::as_str)
        .filter(|cursor| !cursor.is_empty())
        .map(str::to_owned)
}

fn page_query(page: Page) -> Vec<(&'static str, String)> {
    let mut query = vec![("limit", page.limit.to_string())];
    if let Some(after) = page.after {
        query.push(("cursor", after));
    }
    query
}

fn is_conversation_id(value: &str) -> bool {
    matches!(value.as_bytes().first(), Some(b'C' | b'G' | b'D'))
        && value.len() > 1
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn is_user_id(value: &str) -> bool {
    matches!(value.as_bytes().first(), Some(b'U' | b'W'))
        && value.len() > 1
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn is_email(value: &str) -> bool {
    !value.starts_with('@') && value.contains('@')
}

fn trim_colons(name: &str) -> &str {
    name.trim_matches(':')
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn recognizes_ids_and_normalizes_reaction_names() {
        assert!(is_conversation_id("C012AB3CD"));
        assert!(is_conversation_id("D012AB3CD"));
        assert!(!is_conversation_id("eng"));
        assert!(is_user_id("U012AB3CD"));
        assert!(is_email("person@example.com"));
        assert!(!is_email("@person"));
        assert!(is_bot_token("xoxb-secret"));
        assert!(is_bot_token("xoxe.xoxb-rotating-secret"));
        assert!(!is_bot_token("xoxp-user-secret"));
        assert!(is_user_token("xoxp-user-secret"));
        assert!(is_user_token("xoxe.xoxp-rotating-secret"));
        assert!(!is_user_token("xoxb-secret"));
        assert_eq!(trim_colons(":eyes:"), "eyes");
    }

    #[test]
    fn wraps_slack_cursor_pagination() {
        let output = list_output(
            json!({
                "ok": true,
                "channels": [{"id": "C1"}],
                "response_metadata": {"next_cursor": "next-one"}
            }),
            ListShape::Key("channels"),
        )
        .unwrap();
        assert_eq!(output["items"], json!([{"id": "C1"}]));
        assert_eq!(output["pageInfo"]["hasNextPage"], true);
        assert_eq!(output["pageInfo"]["endCursor"], "next-one");
    }

    #[test]
    fn sends_slack_headers_json_and_surfaces_api_errors() {
        let (api, request_rx, server) =
            test_api("200 OK", r#"{"ok":true,"channel":"C1","ts":"1.2"}"#);
        let body = api
            .send(
                Method::POST,
                "chat.postMessage",
                &[],
                Some(json!({"channel": "C1", "text": "hello"})),
            )
            .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /chat.postMessage HTTP/1.1"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret-token")
        );
        assert_eq!(body["ts"], "1.2");
        let (_, request_body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(request_body).unwrap(),
            json!({"channel": "C1", "text": "hello"})
        );

        let (api, _, server) = test_api("200 OK", r#"{"ok":false,"error":"missing_scope"}"#);
        let error = api
            .send(Method::GET, "conversations.list", &[], None)
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(error.to_string(), r#"{"ok":false,"error":"missing_scope"}"#);
    }

    #[test]
    fn validates_identity_with_slack_auth_test() {
        let (api, request_rx, server) = test_api(
            "200 OK",
            r#"{"ok":true,"team":"Acme","team_id":"T1","user":"foac","user_id":"U1","bot_id":"B1"}"#,
        );
        let identity =
            auth_identity_at("status-token", api.base_url.join("auth.test").unwrap()).unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /auth.test http/1.1"));
        assert!(request.contains("authorization: bearer status-token"));
        assert_eq!(identity["team_id"], "T1");

        let (api, _, server) = test_api("200 OK", r#"{"ok":false,"error":"invalid_auth"}"#);
        let error = auth_identity_at("bad", api.base_url.join("auth.test").unwrap()).unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Rejected(_)));

        let (api, _, server) = test_api(
            "200 OK",
            r#"{"ok":true,"team":"Acme","team_id":"T1","user":"person","user_id":"U1"}"#,
        );
        let identity =
            auth_identity_at("xoxp-user", api.base_url.join("auth.test").unwrap()).unwrap();
        server.join().unwrap();
        assert_eq!(identity["user"], "person");
        assert!(identity.get("bot_id").is_none());
    }

    #[test]
    fn resolves_channel_and_user_names() {
        let (api, _, server) = test_api(
            "200 OK",
            r#"{"ok":true,"channels":[{"id":"C123","name":"eng"}],"response_metadata":{"next_cursor":""}}"#,
        );
        assert_eq!(resolve_conversation(&api, "#eng").unwrap(), "C123");
        server.join().unwrap();

        let (api, _, server) = test_api(
            "200 OK",
            r#"{"ok":true,"members":[{"id":"U123","name":"alice","profile":{"display_name":"Alice A"}}],"response_metadata":{"next_cursor":""}}"#,
        );
        assert_eq!(resolve_user(&api, "@alice").unwrap(), "U123");
        server.join().unwrap();
    }

    fn test_api(
        status: &'static str,
        body: &'static str,
    ) -> (Api, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let (url, request_rx, server) = rest::testing::test_server(status, body, "");
        let api = Api {
            client: reqwest::blocking::Client::new(),
            base_url: url,
            token: "secret-token".into(),
            format: crate::output::Format::Json,
        };
        (api, request_rx, server)
    }
}
