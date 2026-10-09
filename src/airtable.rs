//! Airtable provider. REST passthrough against api.airtable.com/v0 with a
//! bearer personal access token. Records and comments are addressed by
//! `--base` and `--table`; base and table metadata live under `/v0/meta`.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest::{self, Api, Auth, BodyInput, insert_opt, read_json};

const BASE_URL: &str = "https://api.airtable.com";

#[derive(Args)]
pub struct Cmd {
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Bases the token can access
    #[command(subcommand)]
    Base(BaseCmd),
    /// Tables in a base, with their fields and views
    #[command(subcommand)]
    Table(TableCmd),
    /// Records (rows) in a table
    #[command(subcommand)]
    Record(RecordCmd),
    /// Comments on a record
    #[command(subcommand)]
    Comment(CommentCmd),
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

#[derive(Args)]
struct TableRef {
    /// Base ID (app...)
    #[arg(long)]
    base: String,
    /// Table ID (tbl...) or name; IDs survive renames
    #[arg(long)]
    table: String,
}

/// Cell values as a JSON object keyed by field name or ID, inline or from a file.
#[derive(Args)]
struct FieldsInput {
    /// Cell values as a JSON object, keyed by field name or ID
    #[arg(
        long,
        conflicts_with = "fields_file",
        required_unless_present = "fields_file"
    )]
    fields: Option<String>,
    /// Read the fields JSON object from a file
    #[arg(long, conflicts_with = "fields")]
    fields_file: Option<PathBuf>,
    /// Convert string values to the field types (select options are created as needed)
    #[arg(long)]
    typecast: bool,
}

#[derive(Subcommand)]
enum BaseCmd {
    /// List bases the token can access
    #[command(after_long_help = outdoc::rest_list("raw Airtable base objects (id, name, permissionLevel)", &["id"], &outdoc::END_CURSOR))]
    List {
        /// Opaque cursor from pageInfo.endCursor
        #[arg(long)]
        after: Option<String>,
    },
}

#[derive(Subcommand)]
enum TableCmd {
    /// List a base's tables with their fields and views (the base schema)
    #[command(after_long_help = outdoc::rest_list("raw Airtable table objects with fields[] and views[]", &["id", "name"], &outdoc::SINGLE_PAGE))]
    List {
        /// Base ID (app...)
        #[arg(long)]
        base: String,
    },
}

#[derive(Subcommand)]
enum RecordCmd {
    /// List records, optionally through a view, a formula, and a sort
    #[command(after_long_help = outdoc::rest_list("raw Airtable record objects (id, createdTime, fields)", &["id"], &outdoc::END_CURSOR))]
    List {
        #[command(flatten)]
        table: TableRef,
        /// View name or ID: only its records, in its order
        #[arg(long)]
        view: Option<String>,
        /// Formula keeping records where it is truthy (filterByFormula)
        #[arg(long)]
        formula: Option<String>,
        /// Sort as a JSON array of {"field": NAME, "direction": "asc"|"desc"}
        #[arg(long)]
        sort: Option<String>,
        /// Only return this field (repeatable)
        #[arg(long = "field", value_name = "FIELD")]
        fields: Vec<String>,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Get a record
    #[command(after_long_help = outdoc::rest_obj("raw Airtable record object", "id"))]
    Get {
        #[command(flatten)]
        table: TableRef,
        /// Record ID (rec...)
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create a record
    #[command(after_long_help = outdoc::rest_obj("raw Airtable record object", "id"))]
    Create {
        #[command(flatten)]
        table: TableRef,
        #[command(flatten)]
        fields: FieldsInput,
    },
    /// Update a record; only the given fields change
    #[command(after_long_help = outdoc::rest_obj("raw Airtable record object", "id"))]
    Update {
        #[command(flatten)]
        table: TableRef,
        /// Record ID (rec...)
        id: String,
        #[command(flatten)]
        fields: FieldsInput,
    },
    /// Delete a record
    #[command(after_long_help = outdoc::rest_obj("{\"id\", \"deleted\": true} object", "id"))]
    Delete {
        #[command(flatten)]
        table: TableRef,
        /// Record ID (rec...)
        id: String,
    },
}

#[derive(Subcommand)]
enum CommentCmd {
    /// List a record's comments, newest first
    #[command(after_long_help = outdoc::rest_list("raw Airtable comment objects", &["id"], &outdoc::END_CURSOR))]
    List {
        #[command(flatten)]
        table: TableRef,
        /// Record ID (rec...)
        record: String,
        #[command(flatten)]
        cursor: Cursor,
    },
    /// Comment on a record, or reply to a comment with --parent
    #[command(after_long_help = outdoc::rest_obj("raw Airtable comment object", "id"))]
    Create {
        #[command(flatten)]
        table: TableRef,
        /// Record ID (rec...)
        record: String,
        #[command(flatten)]
        body: BodyInput,
        /// Comment ID to reply to
        #[arg(long)]
        parent: Option<String>,
    },
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["v0".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let api = api(crate::auth::airtable_token(instance)?, format)?;
    dispatch(&api, cmd.command)
}

fn dispatch(api: &Api, cmd: Resource) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Resource::Base(BaseCmd::List { after }) => {
            let mut query = Vec::new();
            rest::push_query(&mut query, "offset", after);
            let response = api.send(Method::GET, &path!["meta", "bases"], &query, None)?;
            print_wrapped(api, response.body, "bases")
        }
        Resource::Table(TableCmd::List { base }) => {
            let response = api.send(
                Method::GET,
                &path!["meta", "bases", base, "tables"],
                &[],
                None,
            )?;
            print_wrapped(api, response.body, "tables")
        }
        Resource::Record(cmd) => run_record(api, cmd),
        Resource::Comment(cmd) => run_comment(api, cmd),
    }
}

pub fn authenticated() -> bool {
    crate::auth::airtable_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("airtable")
}

pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    let url = reqwest::Url::parse(&format!("{BASE_URL}/v0/meta/whoami"))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        &[],
        &[reqwest::StatusCode::UNAUTHORIZED],
    )
}

fn run_record(api: &Api, cmd: RecordCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        RecordCmd::List {
            table,
            view,
            formula,
            sort,
            fields,
            cursor,
        } => {
            // POST listRecords takes the same options as GET without its
            // 16k-character URL limit or the sort[0][field] query encoding.
            let mut body = Map::new();
            body.insert("pageSize".into(), cursor.limit.into());
            insert_opt(&mut body, "offset", cursor.after);
            insert_opt(&mut body, "view", view);
            insert_opt(&mut body, "filterByFormula", formula);
            insert_opt(&mut body, "sort", rest::parse_json(sort, "--sort")?);
            if !fields.is_empty() {
                body.insert("fields".into(), fields.into());
            }
            let response = api.send(
                Method::POST,
                &path![table.base, table.table, "listRecords"],
                &[],
                Some(body.into()),
            )?;
            print_wrapped(api, response.body, "records")
        }
        RecordCmd::Get { table, id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path![table.base, table.table, id], Vec::new())
        }),
        RecordCmd::Create { table, fields } => api.print(
            Method::POST,
            path![table.base, table.table],
            Vec::new(),
            Some(fields_payload(fields)?),
        ),
        RecordCmd::Update { table, id, fields } => api.print(
            Method::PATCH,
            path![table.base, table.table, id],
            Vec::new(),
            Some(fields_payload(fields)?),
        ),
        RecordCmd::Delete { table, id } => api.print(
            Method::DELETE,
            path![table.base, table.table, id],
            Vec::new(),
            None,
        ),
    }
}

fn fields_payload(input: FieldsInput) -> Result<Value, Box<dyn std::error::Error>> {
    let fields = read_json(input.fields, input.fields_file, "--fields")?
        .filter(Value::is_object)
        .ok_or("--fields must be a JSON object")?;
    let mut payload = json!({ "fields": fields });
    if input.typecast {
        payload["typecast"] = true.into();
    }
    Ok(payload)
}

fn run_comment(api: &Api, cmd: CommentCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommentCmd::List {
            table,
            record,
            cursor,
        } => {
            let mut query = vec![("pageSize", cursor.limit.to_string())];
            rest::push_query(&mut query, "offset", cursor.after);
            let response = api.send(
                Method::GET,
                &path![table.base, table.table, record, "comments"],
                &query,
                None,
            )?;
            print_wrapped(api, response.body, "comments")
        }
        CommentCmd::Create {
            table,
            record,
            body,
            parent,
        } => {
            let mut payload = Map::new();
            payload.insert("text".into(), body.required()?.into());
            insert_opt(&mut payload, "parentCommentId", parent);
            api.print(
                Method::POST,
                path![table.base, table.table, record, "comments"],
                Vec::new(),
                Some(payload.into()),
            )
        }
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

fn print_wrapped(api: &Api, body: Value, key: &str) -> Result<(), Box<dyn std::error::Error>> {
    crate::output::print(&wrap(body, key)?, api.format);
    Ok(())
}

/// Airtable lists are `{"<key>": [...], "offset": "..."}`, the offset present
/// (and non-null) only while more pages exist.
fn wrap(mut body: Value, key: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let items = body[key]
        .as_array_mut()
        .map(std::mem::take)
        .ok_or_else(|| format!("Airtable list response did not contain {key}"))?;
    let cursor = body["offset"].as_str();
    Ok(rest::wrap_list(
        items,
        json!({ "hasNextPage": cursor.is_some(), "endCursor": cursor }),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn table() -> TableRef {
        TableRef {
            base: "app1".into(),
            table: "My Tasks".into(),
        }
    }

    fn fields(json: &str, typecast: bool) -> FieldsInput {
        FieldsInput {
            fields: Some(json.into()),
            fields_file: None,
            typecast,
        }
    }

    fn request_body(request: &str) -> Value {
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
    }

    #[test]
    fn wrap_reads_the_offset_as_the_cursor() {
        let more = wrap(json!({"records": [1], "offset": "itr1/rec2"}), "records").unwrap();
        assert_eq!(
            more,
            json!({"items": [1], "pageInfo": {"hasNextPage": true, "endCursor": "itr1/rec2"}})
        );
        // Comments send `offset: null` on the last page; records omit it.
        for last in [
            json!({"comments": []}),
            json!({"comments": [], "offset": null}),
        ] {
            assert_eq!(
                wrap(last, "comments").unwrap()["pageInfo"],
                json!({"hasNextPage": false, "endCursor": null})
            );
        }
        assert!(wrap(json!({"error": {}}), "records").is_err());
    }

    #[test]
    fn record_list_posts_options_to_list_records_with_bearer_auth() {
        let (api, request_rx, server) = test_api("{\"records\":[]}");
        dispatch(
            &api,
            Resource::Record(RecordCmd::List {
                table: table(),
                view: Some("Grid view".into()),
                formula: Some("{Done} = 0".into()),
                sort: Some(r#"[{"field": "Due", "direction": "desc"}]"#.into()),
                fields: vec!["Name".into(), "Due".into()],
                cursor: Cursor {
                    limit: 10,
                    after: Some("itr1".into()),
                },
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v0/app1/My%20Tasks/listRecords "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer pat-secret")
        );
        assert_eq!(
            request_body(&request),
            json!({
                "pageSize": 10,
                "offset": "itr1",
                "view": "Grid view",
                "filterByFormula": "{Done} = 0",
                "sort": [{"field": "Due", "direction": "desc"}],
                "fields": ["Name", "Due"],
            })
        );
    }

    #[test]
    fn record_create_sends_fields_and_typecast() {
        let (api, request_rx, server) = test_api("{}");
        dispatch(
            &api,
            Resource::Record(RecordCmd::Create {
                table: table(),
                fields: fields(r#"{"Name": "Ship it"}"#, true),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v0/app1/My%20Tasks "));
        assert_eq!(
            request_body(&request),
            json!({"fields": {"Name": "Ship it"}, "typecast": true})
        );
    }

    #[test]
    fn record_update_patches_only_the_given_fields() {
        let (api, request_rx, server) = test_api("{}");
        dispatch(
            &api,
            Resource::Record(RecordCmd::Update {
                table: table(),
                id: "rec1".into(),
                fields: fields(r#"{"Done": true}"#, false),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("PATCH /v0/app1/My%20Tasks/rec1 "));
        assert_eq!(request_body(&request), json!({"fields": {"Done": true}}));
    }

    #[test]
    fn fields_must_be_an_object() {
        let error = fields_payload(fields("[1]", false)).unwrap_err();
        assert_eq!(error.to_string(), "--fields must be a JSON object");
    }

    #[test]
    fn comment_list_pages_through_the_query_string() {
        let (api, request_rx, server) = test_api("{\"comments\":[],\"offset\":null}");
        dispatch(
            &api,
            Resource::Comment(CommentCmd::List {
                table: table(),
                record: "rec1".into(),
                cursor: Cursor {
                    limit: 5,
                    after: Some("c2".into()),
                },
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("GET /v0/app1/My%20Tasks/rec1/comments?pageSize=5&offset=c2 "));
    }

    #[test]
    fn comment_create_replies_to_a_parent() {
        let (api, request_rx, server) = test_api("{}");
        dispatch(
            &api,
            Resource::Comment(CommentCmd::Create {
                table: table(),
                record: "rec1".into(),
                body: BodyInput {
                    body: Some("LGTM".into()),
                    body_file: None,
                },
                parent: Some("com1".into()),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v0/app1/My%20Tasks/rec1/comments "));
        assert_eq!(
            request_body(&request),
            json!({"text": "LGTM", "parentCommentId": "com1"})
        );
    }

    #[test]
    fn table_list_reads_the_base_schema() {
        let (api, request_rx, server) = test_api("{\"tables\":[{\"id\":\"tbl1\"}]}");
        dispatch(
            &api,
            Resource::Table(TableCmd::List {
                base: "app1".into(),
            }),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("GET /v0/meta/bases/app1/tables "));
    }

    fn test_api(body: &str) -> (Api, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let (url, request_rx, server) = rest::testing::test_server("200 OK", body, "");
        let api = Api {
            base_url: url,
            ..api("pat-secret".into(), crate::output::Format::Json).unwrap()
        };
        (api, request_rx, server)
    }
}
