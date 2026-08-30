//! Axiom provider. REST passthrough against api.axiom.co with a bearer
//! token: management resources live on `/v2`, APL queries and ingest on the
//! `/v1` dataset endpoints. Personal access tokens (`xapt-`) also need the
//! organization ID header: --org-id, AXIOM_ORG_ID (default instance), or the
//! ID saved at login.

use std::io::Read;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest::{self, Api, Auth, insert_opt, push_query};

const DEFAULT_URL: &str = "https://api.axiom.co";
const ORG_HEADER: &str = "X-Axiom-Org-Id";

#[derive(Args)]
pub struct Cmd {
    /// Organization ID, needed with a personal access token (xapt-);
    /// defaults to AXIOM_ORG_ID, then the ID saved by `foac auth axiom login`
    #[arg(long, global = true)]
    org_id: Option<String>,
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Datasets
    #[command(subcommand)]
    Dataset(DatasetCmd),
    /// Fields of a dataset
    #[command(subcommand)]
    Field(FieldCmd),
    /// Run an APL query
    #[command(after_long_help = outdoc::lines(&[
        r#"{"items": [<row>, ...], "pageInfo": {"hasNextPage": true, "minCursor": "...", "maxCursor": "..."}, "status": {...}}"#,
        "Records: items[], one object per result row keyed by the query's output fields (Axiom's columnar tables transposed)",
        "Next page: sort by _time in the query, then pass pageInfo.maxCursor (ascending) or pageInfo.minCursor (descending) to --cursor while hasNextPage is true (false once a page has no rows past the cursor)",
        "status is Axiom's raw query status (rowsMatched, elapsedTime, messages, ...)",
    ]))]
    Query(QueryArgs),
    /// Ingest events into a dataset
    #[command(after_long_help = outdoc::lines(&[
        r#"{"ingested": N, "failed": N, "failures": [...], "processedBytes": N, ...}"#,
        "Raw Axiom ingest status; foac adds no envelope",
    ]))]
    Ingest(IngestArgs),
    /// Annotations (deploys, incidents, ...) shown on dataset charts
    #[command(subcommand)]
    Annotation(AnnotationCmd),
    /// Monitors
    #[command(subcommand)]
    Monitor(MonitorCmd),
    /// Notifiers
    #[command(subcommand)]
    Notifier(NotifierCmd),
    /// Users of the organization
    #[command(subcommand)]
    User(UserCmd),
    /// Organizations the token can see
    #[command(subcommand)]
    Org(OrgCmd),
}

#[derive(Args)]
struct QueryArgs {
    /// APL query, e.g. "['logs'] | where level == 'error' | limit 20"
    apl: String,
    /// Query window start, RFC 3339 (or use ago() in the query)
    #[arg(long)]
    start_time: Option<String>,
    /// Query window end, RFC 3339
    #[arg(long)]
    end_time: Option<String>,
    /// Cursor from a previous result's pageInfo.minCursor/maxCursor
    #[arg(long)]
    cursor: Option<String>,
    /// Include the event the cursor points at
    #[arg(long)]
    include_cursor: bool,
}

#[derive(Args)]
struct IngestArgs {
    /// Dataset name
    dataset: String,
    #[command(flatten)]
    events: EventsInput,
    /// Event field holding the timestamp (default _time)
    #[arg(long)]
    timestamp_field: Option<String>,
    /// Go reference-time layout of the timestamp field
    #[arg(long)]
    timestamp_format: Option<String>,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct EventsInput {
    /// Events as a JSON array, one object, or NDJSON
    #[arg(long)]
    events: Option<String>,
    /// Read the events from a file (- for stdin)
    #[arg(long)]
    events_file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum DatasetCmd {
    /// List datasets
    #[command(after_long_help = outdoc::rest_list("raw Axiom dataset objects", &["name"], &outdoc::SINGLE_PAGE))]
    List,
    /// Get a dataset by name
    #[command(after_long_help = outdoc::rest_obj("raw Axiom dataset object", "name (also its id)"))]
    Get {
        name: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create a dataset
    #[command(after_long_help = outdoc::rest_obj("raw Axiom dataset object", "name (also its id)"))]
    Create {
        name: String,
        #[arg(long)]
        description: Option<String>,
        /// Dataset kind, e.g. events
        #[arg(long)]
        kind: Option<String>,
        /// Retain events for this many days instead of the plan default
        #[arg(long)]
        retention_days: Option<u32>,
    },
    /// Update a dataset; only supplied fields are sent
    #[command(after_long_help = outdoc::rest_obj("raw Axiom dataset object", "name (also its id)"))]
    Update {
        name: String,
        #[arg(long)]
        description: Option<String>,
        /// Retain events for this many days instead of the plan default
        #[arg(long)]
        retention_days: Option<u32>,
    },
    /// Delete a dataset and all its events
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { name: String },
    /// Drop events older than a duration
    #[command(after_long_help = outdoc::lines(&[
        "Raw Axiom trim result, or {} when Axiom returns no body",
    ]))]
    Trim {
        name: String,
        /// Oldest events to keep, as a Go duration like 720h or 30m
        #[arg(long)]
        max_duration: String,
    },
}

#[derive(Subcommand)]
enum FieldCmd {
    /// List a dataset's fields
    #[command(after_long_help = outdoc::rest_list("raw Axiom field objects", &["name"], &outdoc::SINGLE_PAGE))]
    List {
        /// Dataset name
        #[arg(long)]
        dataset: String,
    },
    /// Get one field of a dataset
    #[command(after_long_help = outdoc::rest_obj("raw Axiom field object", "name"))]
    Get {
        name: Option<String>,
        /// Dataset name
        #[arg(long)]
        dataset: String,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Args)]
struct AnnotationFields {
    /// Datasets the annotation belongs to (repeatable)
    #[arg(long = "dataset")]
    datasets: Vec<String>,
    /// Annotation type: lowercase letters, digits, and hyphens
    #[arg(long = "type", value_name = "TYPE")]
    kind: Option<String>,
    /// Start time, RFC 3339 (default now)
    #[arg(long)]
    time: Option<String>,
    /// End time, RFC 3339
    #[arg(long)]
    end_time: Option<String>,
    /// Short summary shown on charts
    #[arg(long)]
    title: Option<String>,
    /// Longer explanation of the marked event
    #[arg(long)]
    description: Option<String>,
    /// Link shown with the annotation, e.g. a pull request
    #[arg(long)]
    url: Option<String>,
}

#[derive(Subcommand)]
enum AnnotationCmd {
    /// List annotations
    #[command(after_long_help = outdoc::rest_list("raw Axiom annotation objects", &["id"], &outdoc::SINGLE_PAGE))]
    List {
        /// Only annotations on these datasets (repeatable)
        #[arg(long = "dataset")]
        datasets: Vec<String>,
        /// Only annotations after this time, RFC 3339
        #[arg(long)]
        start: Option<String>,
        /// Only annotations before this time, RFC 3339
        #[arg(long)]
        end: Option<String>,
    },
    /// Get an annotation by ID
    #[command(after_long_help = outdoc::rest_obj("raw Axiom annotation object", "id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create an annotation; --type and at least one --dataset are required
    #[command(after_long_help = outdoc::rest_obj("raw Axiom annotation object", "id"))]
    Create {
        #[command(flatten)]
        fields: AnnotationFields,
    },
    /// Update an annotation; only supplied fields are sent
    #[command(after_long_help = outdoc::rest_obj("raw Axiom annotation object", "id"))]
    Update {
        id: String,
        #[command(flatten)]
        fields: AnnotationFields,
    },
    /// Delete an annotation
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { id: String },
}

#[derive(Subcommand)]
enum MonitorCmd {
    /// List monitors
    #[command(after_long_help = outdoc::rest_list("raw Axiom monitor objects", &["id"], &outdoc::SINGLE_PAGE))]
    List,
    /// Get a monitor by ID
    #[command(after_long_help = outdoc::rest_obj("raw Axiom monitor object", "id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Alert history of a monitor in a time window
    #[command(after_long_help = outdoc::rest_list("raw Axiom alert history entries", &[], &outdoc::SINGLE_PAGE))]
    History {
        id: String,
        /// Window start, RFC 3339
        #[arg(long)]
        start_time: String,
        /// Window end, RFC 3339
        #[arg(long)]
        end_time: String,
    },
}

#[derive(Subcommand)]
enum NotifierCmd {
    /// List notifiers
    #[command(after_long_help = outdoc::rest_list("raw Axiom notifier objects", &["id"], &outdoc::SINGLE_PAGE))]
    List,
    /// Get a notifier by ID
    #[command(after_long_help = outdoc::rest_obj("raw Axiom notifier object", "id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
enum UserCmd {
    /// List the organization's users
    #[command(after_long_help = outdoc::rest_list("raw Axiom user objects", &["id", "email"], &outdoc::SINGLE_PAGE))]
    List,
    /// Get a user by ID
    #[command(after_long_help = outdoc::rest_obj("raw Axiom user object", "id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
enum OrgCmd {
    /// List organizations
    #[command(after_long_help = outdoc::rest_list("raw Axiom organization objects", &["id"], &outdoc::SINGLE_PAGE))]
    List,
    /// Get an organization by ID
    #[command(after_long_help = outdoc::rest_obj("raw Axiom organization object", "id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {
        vec![$($segment.to_string()),*]
    };
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Cmd {
        org_id: flag,
        command,
    } = cmd;
    let api = api(
        crate::auth::axiom_token(instance)?,
        flag.or_else(|| org_id(instance)),
        format,
        instance,
    )?;
    match command {
        Resource::Dataset(cmd) => run_dataset(&api, cmd),
        Resource::Field(cmd) => run_field(&api, cmd),
        Resource::Query(args) => run_query(&api, args),
        Resource::Ingest(args) => run_ingest(&api, args),
        Resource::Annotation(cmd) => run_annotation(&api, cmd),
        Resource::Monitor(cmd) => run_monitor(&api, cmd),
        Resource::Notifier(cmd) => run_simple(&api, "notifiers", cmd.into()),
        Resource::User(cmd) => run_simple(&api, "users", cmd.into()),
        Resource::Org(cmd) => run_simple(&api, "orgs", cmd.into()),
    }
}

pub fn authenticated() -> bool {
    crate::auth::axiom_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("axiom")
}

pub(crate) fn auth_identity(
    token: &str,
    org_override: Option<&str>,
    instance: &str,
) -> Result<Value, crate::auth::ValidationError> {
    // Not /v2/user: an API token is not a user, and Axiom answers that
    // endpoint with a 500 for one. Every useful foac token can read datasets.
    let url = reqwest::Url::parse(&format!("{}/v2/datasets", base_url(instance)))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    let org_id = org_override.map(str::to_owned).or_else(|| org_id(instance));
    let headers: Vec<(&str, &str)> = org_id.iter().map(|id| (ORG_HEADER, id.as_str())).collect();
    rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        &headers,
        &[
            reqwest::StatusCode::FORBIDDEN,
            reqwest::StatusCode::UNAUTHORIZED,
        ],
    )
}

fn run_dataset(api: &Api, cmd: DatasetCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DatasetCmd::List => print_list(api, path!["v2", "datasets"], Vec::new()),
        DatasetCmd::Get { name, from } => pipe::run_get(name, from, api.format, |name| {
            api.get_body(path!["v2", "datasets", name], Vec::new())
        }),
        DatasetCmd::Create {
            name,
            description,
            kind,
            retention_days,
        } => {
            let mut payload = Map::new();
            payload.insert("name".into(), json!(name));
            payload.insert("description".into(), json!(description.unwrap_or_default()));
            insert_opt(&mut payload, "kind", kind);
            insert_retention(&mut payload, retention_days);
            api.print(
                Method::POST,
                path!["v2", "datasets"],
                Vec::new(),
                Some(payload.into()),
            )
        }
        DatasetCmd::Update {
            name,
            description,
            retention_days,
        } => {
            let mut payload = Map::new();
            insert_opt(&mut payload, "description", description);
            insert_retention(&mut payload, retention_days);
            api.print(
                Method::PUT,
                path!["v2", "datasets", name],
                Vec::new(),
                Some(payload.into()),
            )
        }
        DatasetCmd::Delete { name } => api.print(
            Method::DELETE,
            path!["v2", "datasets", name],
            Vec::new(),
            None,
        ),
        DatasetCmd::Trim { name, max_duration } => api.print(
            Method::POST,
            path!["v2", "datasets", name, "trim"],
            Vec::new(),
            Some(json!({ "maxDuration": max_duration })),
        ),
    }
}

fn run_field(api: &Api, cmd: FieldCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        FieldCmd::List { dataset } => {
            print_list(api, path!["v2", "datasets", dataset, "fields"], Vec::new())
        }
        FieldCmd::Get {
            name,
            dataset,
            from,
        } => pipe::run_get(name, from, api.format, |name| {
            api.get_body(path!["v2", "datasets", dataset, "fields", name], Vec::new())
        }),
    }
}

fn run_query(api: &Api, args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut payload = Map::new();
    payload.insert("apl".into(), json!(args.apl));
    insert_opt(&mut payload, "startTime", args.start_time);
    insert_opt(&mut payload, "endTime", args.end_time);
    insert_opt(&mut payload, "cursor", args.cursor.clone());
    if args.include_cursor {
        payload.insert("includeCursor".into(), json!(true));
    }
    let response = api.send(
        Method::POST,
        &["v1".to_owned(), "datasets".to_owned(), "_apl".to_owned()],
        &[("format", "tabular".to_owned())],
        Some(payload.into()),
    )?;
    crate::output::print(
        &query_result(response.body, args.cursor.as_deref()),
        api.format,
    );
    Ok(())
}

/// Axiom's tabular result stores each table column-major (`fields[]` names,
/// `columns[c][r]` values); rows as objects are what a reader wants.
///
/// Axiom ends a cursor walk with an empty page, or (with `includeCursor`)
/// a page holding only the cursor event, so both cursors equal `sent`.
/// `rowsMatched` is the whole-window count and says nothing about paging.
fn query_result(body: Value, sent: Option<&str>) -> Value {
    let rows: Vec<Value> = body["tables"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(table_rows)
        .collect();
    let status = &body["status"];
    let cursor = |key: &str| status[key].as_str().filter(|cursor| !cursor.is_empty());
    let (min, max) = (cursor("minCursor"), cursor("maxCursor"));
    let has_next = !rows.is_empty() && max.is_some() && !(min == sent && max == sent);
    let mut result = rest::wrap_list(
        rows,
        json!({
            "hasNextPage": has_next,
            "minCursor": status["minCursor"],
            "maxCursor": status["maxCursor"],
        }),
    );
    result["status"] = status.clone();
    result
}

fn table_rows(table: &Value) -> Vec<Value> {
    let names: Vec<&str> = table["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|field| field["name"].as_str())
        .collect();
    let columns: Vec<&Vec<Value>> = table["columns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .collect();
    let count = columns.first().map_or(0, |column| column.len());
    (0..count)
        .map(|row| {
            names
                .iter()
                .zip(&columns)
                .map(|(name, column)| ((*name).to_owned(), column[row].clone()))
                .collect::<Map<String, Value>>()
                .into()
        })
        .collect()
}

fn run_ingest(api: &Api, args: IngestArgs) -> Result<(), Box<dyn std::error::Error>> {
    let events = parse_events(&args.events.read()?)?;
    let mut query = Vec::new();
    push_query(&mut query, "timestamp-field", args.timestamp_field);
    push_query(&mut query, "timestamp-format", args.timestamp_format);
    api.print(
        Method::POST,
        path!["v1", "datasets", args.dataset, "ingest"],
        query,
        Some(Value::Array(events)),
    )
}

fn run_annotation(api: &Api, cmd: AnnotationCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        AnnotationCmd::List {
            datasets,
            start,
            end,
        } => {
            let mut query: Vec<(&'static str, String)> =
                datasets.into_iter().map(|d| ("datasets", d)).collect();
            push_query(&mut query, "start", start);
            push_query(&mut query, "end", end);
            print_list(api, path!["v2", "annotations"], query)
        }
        AnnotationCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["v2", "annotations", id], Vec::new())
        }),
        AnnotationCmd::Create { fields } => {
            if fields.kind.is_none() || fields.datasets.is_empty() {
                return Err("--type and at least one --dataset are required".into());
            }
            api.print(
                Method::POST,
                path!["v2", "annotations"],
                Vec::new(),
                Some(annotation_payload(fields).into()),
            )
        }
        AnnotationCmd::Update { id, fields } => api.print(
            Method::PUT,
            path!["v2", "annotations", id],
            Vec::new(),
            Some(annotation_payload(fields).into()),
        ),
        AnnotationCmd::Delete { id } => api.print(
            Method::DELETE,
            path!["v2", "annotations", id],
            Vec::new(),
            None,
        ),
    }
}

fn run_monitor(api: &Api, cmd: MonitorCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        MonitorCmd::List => print_list(api, path!["v2", "monitors"], Vec::new()),
        MonitorCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["v2", "monitors", id], Vec::new())
        }),
        MonitorCmd::History {
            id,
            start_time,
            end_time,
        } => print_list(
            api,
            path!["v2", "monitors", id, "history"],
            vec![("startTime", start_time), ("endTime", end_time)],
        ),
    }
}

/// The read-only resources share one list/get shape.
enum SimpleCmd {
    List,
    Get(Option<String>, FromFlag),
}

impl From<NotifierCmd> for SimpleCmd {
    fn from(cmd: NotifierCmd) -> Self {
        match cmd {
            NotifierCmd::List => Self::List,
            NotifierCmd::Get { id, from } => Self::Get(id, from),
        }
    }
}

impl From<UserCmd> for SimpleCmd {
    fn from(cmd: UserCmd) -> Self {
        match cmd {
            UserCmd::List => Self::List,
            UserCmd::Get { id, from } => Self::Get(id, from),
        }
    }
}

impl From<OrgCmd> for SimpleCmd {
    fn from(cmd: OrgCmd) -> Self {
        match cmd {
            OrgCmd::List => Self::List,
            OrgCmd::Get { id, from } => Self::Get(id, from),
        }
    }
}

fn run_simple(api: &Api, resource: &str, cmd: SimpleCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        SimpleCmd::List => print_list(api, path!["v2", resource], Vec::new()),
        SimpleCmd::Get(id, from) => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["v2", resource, id], Vec::new())
        }),
    }
}

impl EventsInput {
    fn read(self) -> Result<String, Box<dyn std::error::Error>> {
        match (self.events, self.events_file) {
            (Some(events), None) => Ok(events),
            (None, Some(path)) if path.as_os_str() == "-" => {
                let mut input = String::new();
                std::io::stdin().read_to_string(&mut input)?;
                Ok(input)
            }
            (None, Some(path)) => Ok(std::fs::read_to_string(&path)
                .map_err(|error| format!("--events-file {}: {error}", path.display()))?),
            _ => unreachable!("clap enforces --events xor --events-file"),
        }
    }
}

/// One JSON array, one object, or NDJSON (any whitespace-separated JSON
/// values), flattened into the array Axiom ingests.
fn parse_events(text: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut events = Vec::new();
    for value in serde_json::Deserializer::from_str(text).into_iter::<Value>() {
        match value.map_err(|error| format!("events are not valid JSON: {error}"))? {
            Value::Array(items) => events.extend(items),
            object @ Value::Object(_) => events.push(object),
            other => return Err(format!("events must be JSON objects, got {other}").into()),
        }
    }
    if events.is_empty() {
        return Err("no events to ingest".into());
    }
    Ok(events)
}

fn insert_retention(payload: &mut Map<String, Value>, days: Option<u32>) {
    if let Some(days) = days {
        payload.insert("useRetentionPeriod".into(), json!(true));
        payload.insert("retentionDays".into(), json!(days));
    }
}

fn annotation_payload(fields: AnnotationFields) -> Map<String, Value> {
    let mut payload = Map::new();
    if !fields.datasets.is_empty() {
        payload.insert("datasets".into(), json!(fields.datasets));
    }
    insert_opt(&mut payload, "type", fields.kind);
    insert_opt(&mut payload, "time", fields.time);
    insert_opt(&mut payload, "endTime", fields.end_time);
    insert_opt(&mut payload, "title", fields.title);
    insert_opt(&mut payload, "description", fields.description);
    insert_opt(&mut payload, "url", fields.url);
    payload
}

fn api(
    token: String,
    org_id: Option<String>,
    format: crate::output::Format,
    instance: &str,
) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: client(org_id)?,
        base_url: reqwest::Url::parse(&base_url(instance))?,
        auth: Auth::Bearer(token),
        format,
        headers: &[],
        trailing_slash: false,
    })
}

/// The org header is per invocation, so it rides on the client rather than
/// on `Api::headers`, which is static.
fn client(org_id: Option<String>) -> Result<reqwest::blocking::Client, Box<dyn std::error::Error>> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(org_id) = org_id {
        headers.insert(ORG_HEADER, org_id.parse()?);
    }
    Ok(reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()?)
}

fn print_list(
    api: &Api,
    segments: Vec<String>,
    query: Vec<(&'static str, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = api.send(Method::GET, &segments, &query, None)?;
    let items = response
        .body
        .as_array()
        .cloned()
        .ok_or("Axiom list response was not an array")?;
    crate::output::print(
        &rest::wrap_list(items, json!({ "hasNextPage": false })),
        api.format,
    );
    Ok(())
}

/// `AXIOM_ORG_ID` for the default instance, else the org ID saved at login.
fn org_id(instance: &str) -> Option<String> {
    let environment = if instance == crate::provider::DEFAULT_INSTANCE {
        crate::auth::environment("AXIOM_ORG_ID")
    } else {
        None
    };
    environment
        .or_else(|| crate::auth::stored_value(crate::provider::Credential::AxiomOrg, instance))
}

/// `AXIOM_URL` for the default instance, else the cloud API.
pub(crate) fn base_url(instance: &str) -> String {
    let environment = if instance == crate::provider::DEFAULT_INSTANCE {
        crate::auth::environment("AXIOM_URL")
    } else {
        None
    };
    resolve_base_url(environment)
}

fn resolve_base_url(environment: Option<String>) -> String {
    environment
        .map(|url| url.trim().trim_end_matches('/').to_owned())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn query_posts_apl_and_transposes_the_tabular_result() {
        let (api, request_rx, server) = test_api("200 OK", "{\"status\":{}}", None);
        run_query(
            &api,
            QueryArgs {
                apl: "['logs'] | limit 1".into(),
                start_time: Some("2026-01-01T00:00:00Z".into()),
                end_time: None,
                cursor: None,
                include_cursor: false,
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v1/datasets/_apl?format=tabular HTTP/1.1"));
        assert!(request.contains("authorization: Bearer secret-token"));
        assert!(!request.contains("x-axiom-org-id"));
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "apl": "['logs'] | limit 1", "startTime": "2026-01-01T00:00:00Z" })
        );
    }

    #[test]
    fn query_result_turns_columns_into_row_objects() {
        let body = json!({
            "status": { "rowsMatched": 5, "minCursor": "min", "maxCursor": "max" },
            "tables": [{
                "name": "0",
                "fields": [{ "name": "_time" }, { "name": "level" }],
                "columns": [["t1", "t2"], ["error", null]],
            }],
        });
        assert_eq!(
            query_result(body, None),
            json!({
                "items": [
                    { "_time": "t1", "level": "error" },
                    { "_time": "t2", "level": null },
                ],
                "pageInfo": { "hasNextPage": true, "minCursor": "min", "maxCursor": "max" },
                "status": { "rowsMatched": 5, "minCursor": "min", "maxCursor": "max" },
            })
        );
        let empty = query_result(
            json!({ "status": { "rowsMatched": 0 }, "tables": [{ "fields": [], "columns": [] }] }),
            None,
        );
        assert_eq!(empty["items"], json!([]));
        assert_eq!(empty["pageInfo"]["hasNextPage"], false);
        assert_eq!(query_result(json!({}), None)["items"], json!([]));
    }

    #[test]
    fn has_next_page_follows_axiom_cursor_end_signals_not_rows_matched() {
        let page = |min: &str, max: &str, rows: usize| {
            json!({
                "status": { "rowsMatched": 2500, "minCursor": min, "maxCursor": max },
                "tables": [{ "fields": [{ "name": "_time" }], "columns": [vec!["t"; rows]] }],
            })
        };
        // A full page mid-walk: more to fetch.
        assert_eq!(
            query_result(page("c6", "c9", 4), Some("c5"))["pageInfo"]["hasNextPage"],
            true
        );
        // Ascending with --include-cursor: minCursor echoes the sent cursor.
        assert_eq!(
            query_result(page("c5", "c9", 5), Some("c5"))["pageInfo"]["hasNextPage"],
            true
        );
        // The last page is empty, or only the cursor event itself.
        assert_eq!(
            query_result(page("", "", 0), Some("c9"))["pageInfo"]["hasNextPage"],
            false
        );
        assert_eq!(
            query_result(page("c9", "c9", 1), Some("c9"))["pageInfo"]["hasNextPage"],
            false
        );
        // An aggregation reports no cursors: nothing to walk.
        let summary = json!({
            "status": { "rowsMatched": 100000 },
            "tables": [{ "fields": [{ "name": "count_" }], "columns": [[3]] }],
        });
        assert_eq!(
            query_result(summary, None)["pageInfo"]["hasNextPage"],
            false
        );
    }

    #[test]
    fn org_header_rides_on_the_client_when_configured() {
        let (api, request_rx, server) = test_api("200 OK", "[]", Some("org-1"));
        run_dataset(&api, DatasetCmd::List).unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /v2/datasets http/1.1"));
        assert!(request.contains("x-axiom-org-id: org-1"));
    }

    #[test]
    fn parse_events_accepts_arrays_objects_and_ndjson() {
        let expected = vec![json!({ "a": 1 }), json!({ "b": 2 })];
        assert_eq!(parse_events(r#"[{"a":1},{"b":2}]"#).unwrap(), expected);
        assert_eq!(parse_events("{\"a\":1}\n{\"b\":2}\n").unwrap(), expected);
        assert_eq!(parse_events(r#"{"a":1}"#).unwrap(), vec![json!({ "a": 1 })]);
        assert!(
            parse_events("")
                .unwrap_err()
                .to_string()
                .contains("no events")
        );
        assert!(
            parse_events("42")
                .unwrap_err()
                .to_string()
                .contains("objects")
        );
        assert!(
            parse_events("{")
                .unwrap_err()
                .to_string()
                .contains("not valid JSON")
        );
    }

    #[test]
    fn ingest_posts_the_flattened_events_with_timestamp_params() {
        let (api, request_rx, server) = test_api("200 OK", "{\"ingested\":2}", None);
        run_ingest(
            &api,
            IngestArgs {
                dataset: "logs".into(),
                events: EventsInput {
                    events: Some("{\"a\":1}\n{\"b\":2}".into()),
                    events_file: None,
                },
                timestamp_field: Some("ts".into()),
                timestamp_format: None,
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v1/datasets/logs/ingest?timestamp-field=ts HTTP/1.1"));
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body, json!([{ "a": 1 }, { "b": 2 }]));
    }

    #[test]
    fn annotation_list_repeats_the_datasets_parameter() {
        let (api, request_rx, server) = test_api("200 OK", "[{\"id\":\"a1\"}]", None);
        run_annotation(
            &api,
            AnnotationCmd::List {
                datasets: vec!["logs".into(), "traces".into()],
                start: Some("2026-01-01T00:00:00Z".into()),
                end: None,
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with(
            "GET /v2/annotations?datasets=logs&datasets=traces&start=2026-01-01T00%3A00%3A00Z HTTP/1.1"
        ));
    }

    #[test]
    fn annotation_create_requires_type_and_dataset() {
        let (api, _, _) = test_api("200 OK", "{}", None);
        let error = run_annotation(
            &api,
            AnnotationCmd::Create {
                fields: AnnotationFields {
                    datasets: vec![],
                    kind: Some("deploy".into()),
                    time: None,
                    end_time: None,
                    title: None,
                    description: None,
                    url: None,
                },
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("--dataset"));
    }

    #[test]
    fn dataset_create_sends_retention_only_when_asked() {
        let (api, request_rx, server) = test_api("200 OK", "{}", None);
        run_dataset(
            &api,
            DatasetCmd::Create {
                name: "logs".into(),
                description: None,
                kind: None,
                retention_days: Some(30),
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "name": "logs", "description": "", "useRetentionPeriod": true, "retentionDays": 30 })
        );
        assert_eq!(
            Value::from(annotation_payload(AnnotationFields {
                datasets: vec![],
                kind: None,
                time: None,
                end_time: None,
                title: Some("v1".into()),
                description: None,
                url: None,
            })),
            json!({ "title": "v1" })
        );
    }

    #[test]
    fn base_url_prefers_the_environment_then_the_cloud_api() {
        assert_eq!(
            resolve_base_url(Some("https://axiom.example.com/".into())),
            "https://axiom.example.com"
        );
        assert_eq!(resolve_base_url(Some("  ".into())), DEFAULT_URL);
        assert_eq!(resolve_base_url(None), DEFAULT_URL);
    }

    fn test_api(
        status: &str,
        body: &str,
        org_id: Option<&str>,
    ) -> (Api, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let (url, request_rx, server) = rest::testing::test_server(status, body, "");
        let api = Api {
            client: client(org_id.map(str::to_owned)).unwrap(),
            base_url: url,
            auth: Auth::Bearer("secret-token".into()),
            format: crate::output::Format::Json,
            headers: &[],
            trailing_slash: false,
        };
        (api, request_rx, server)
    }
}
