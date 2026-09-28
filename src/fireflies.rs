//! Fireflies provider. GraphQL against api.fireflies.ai with a bearer API
//! key; operations are compile-time checked against the vendored schema,
//! like Linear's. Fireflies lists are bare arrays with `limit`/`skip`
//! arguments, so foac wraps them in the REST `{items, pageInfo}` envelope.

use clap::{Args, Subcommand};
use graphql_client::GraphQLQuery;
use serde_json::{Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest;

const API_URL: &str = "https://api.fireflies.ai/graphql";

// Fireflies' one custom scalar, kept as the ISO 8601 string the user typed.
type DateTime = String;

macro_rules! fireflies_query {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "assets/graphql/fireflies/schema.graphql",
            query_path = "assets/graphql/fireflies/queries.graphql",
            variables_derives = "Clone, Default",
            skip_serializing_none
        )]
        struct $name;
    )+};
}

fireflies_query!(
    UserGet,
    UserList,
    TranscriptList,
    TranscriptGet,
    TranscriptUpdate,
    TranscriptDelete,
    BiteList,
    BiteGet,
    ChannelList,
    ChannelGet,
    ContactList,
    ThreadList,
    ThreadGet,
    ThreadCreate,
    ThreadContinue,
    ThreadDelete,
);

// Upload input nests a required enum (download auth type), so it cannot
// derive Default and spells out every field instead.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "assets/graphql/fireflies/schema.graphql",
    query_path = "assets/graphql/fireflies/queries.graphql",
    variables_derives = "Clone",
    skip_serializing_none
)]
struct TranscriptCreate;

#[derive(Subcommand)]
pub enum Cmd {
    /// Meeting transcripts: summaries, attendees, and what was said
    #[command(subcommand)]
    Transcript(TranscriptCmd),
    /// AskFred threads: questions answered from your meetings
    #[command(subcommand)]
    Askfred(AskfredCmd),
    /// Bites: clips cut from a transcript
    #[command(subcommand)]
    Bite(BiteCmd),
    /// Channels that group meetings
    #[command(subcommand)]
    Channel(ChannelCmd),
    /// People you have met with
    #[command(subcommand)]
    Contact(ContactCmd),
    /// Users on your Fireflies team
    #[command(subcommand)]
    User(UserCmd),
}

#[derive(Args)]
pub struct Page {
    /// Results per page; Fireflies allows at most 50
    #[arg(long, default_value_t = 50)]
    limit: u32,
    /// Zero-based index of the first result
    #[arg(long, default_value_t = 0)]
    start_at: u32,
}

#[derive(Subcommand)]
pub enum TranscriptCmd {
    /// List transcripts, newest first
    #[command(after_long_help = outdoc::rest_list("Fireflies transcript objects (no summary or sentences; use get)", &["id"], &outdoc::NEXT_START_AT))]
    List {
        /// Search meeting titles (see --scope)
        #[arg(long)]
        keyword: Option<String>,
        /// Where --keyword searches; Fireflies defaults to title
        #[arg(long, requires = "keyword", value_parser = ["title", "sentences", "all"])]
        scope: Option<String>,
        /// Only transcripts created at or after this ISO 8601 time, like 2026-01-31T00:00:00.000Z
        #[arg(long)]
        from_date: Option<String>,
        /// Only transcripts created before this ISO 8601 time
        #[arg(long)]
        to_date: Option<String>,
        /// Only meetings organized by this email; repeatable
        #[arg(long = "organizer")]
        organizers: Vec<String>,
        /// Only meetings attended by this email; repeatable
        #[arg(long = "participant")]
        participants: Vec<String>,
        /// Only meetings this user ID organized or attended
        #[arg(long)]
        user: Option<String>,
        /// Only meetings in this channel ID
        #[arg(long)]
        channel: Option<String>,
        /// Only meetings the API key's owner organized
        #[arg(long)]
        mine: bool,
        #[command(flatten)]
        page: Page,
    },
    /// Get one transcript: attendees, summary, action items, and optionally what was said
    #[command(after_long_help = outdoc::fireflies("transcript", "transcript.id"))]
    Get {
        id: Option<String>,
        /// Include the spoken transcript as `sentences` (large)
        #[arg(long)]
        sentences: bool,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Transcribe an audio or video file from a public URL (paid plans)
    #[command(after_long_help = outdoc::fireflies("uploadAudio", ""))]
    Create {
        /// Publicly reachable URL of the media file
        #[arg(long)]
        url: String,
        /// Meeting title
        #[arg(long)]
        title: Option<String>,
    },
    /// Rename a transcript (admins only)
    #[command(after_long_help = outdoc::fireflies("updateMeetingTitle", "updateMeetingTitle.id"))]
    Update {
        id: String,
        /// New title
        #[arg(long)]
        title: String,
    },
    /// Delete a transcript
    #[command(after_long_help = outdoc::fireflies("deleteTranscript", "deleteTranscript.id"))]
    Delete { id: String },
}

#[derive(Subcommand)]
pub enum AskfredCmd {
    /// List your AskFred threads
    #[command(after_long_help = outdoc::rest_list("Fireflies AskFred thread summaries", &["id"], &outdoc::SINGLE_PAGE))]
    List {
        /// Only threads about this transcript ID
        #[arg(long)]
        transcript: Option<String>,
    },
    /// Get a thread with all its messages
    #[command(after_long_help = outdoc::fireflies("askfred_thread", "askfred_thread.id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Ask a question in a new thread, about one transcript or across meetings
    #[command(after_long_help = outdoc::fireflies("createAskFredThread", "createAskFredThread.message.thread_id; the answer is createAskFredThread.message.answer"))]
    Create {
        /// The question, up to 2000 characters
        question: String,
        /// Answer from this transcript ID only
        #[arg(long, conflicts_with_all = ["start_time", "end_time", "organizers", "participants", "channels"])]
        transcript: Option<String>,
        /// Earliest meeting time (ISO 8601, at most a year back); Fireflies
        /// defaults to 30 days before --end-time
        #[arg(long)]
        start_time: Option<String>,
        /// Latest meeting time (ISO 8601); Fireflies defaults to now
        #[arg(long)]
        end_time: Option<String>,
        /// Only meetings organized by this email; repeatable
        #[arg(long = "organizer")]
        organizers: Vec<String>,
        /// Only meetings attended by this email; repeatable
        #[arg(long = "participant")]
        participants: Vec<String>,
        /// Only meetings in this channel ID; repeatable
        #[arg(long = "channel")]
        channels: Vec<String>,
    },
    /// Ask a follow-up question in an existing thread
    #[command(after_long_help = outdoc::fireflies("continueAskFredThread", "continueAskFredThread.message.thread_id; the answer is continueAskFredThread.message.answer"))]
    Continue {
        /// Thread ID
        id: String,
        /// The question, up to 2000 characters
        question: String,
    },
    /// Delete a thread and its messages
    #[command(after_long_help = outdoc::fireflies("deleteAskFredThread", "deleteAskFredThread.id"))]
    Delete { id: String },
}

#[derive(Subcommand)]
pub enum BiteCmd {
    /// List bites; your own unless --transcript or --team is given
    #[command(after_long_help = outdoc::rest_list("Fireflies bite objects", &["id"], &outdoc::NEXT_START_AT))]
    List {
        /// Only bites cut from this transcript ID
        #[arg(long)]
        transcript: Option<String>,
        /// Bites across your team
        #[arg(long)]
        team: bool,
        #[command(flatten)]
        page: Page,
    },
    /// Get one bite with its captions and media sources
    #[command(after_long_help = outdoc::fireflies("bite", "bite.id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
pub enum ChannelCmd {
    /// List channels
    #[command(after_long_help = outdoc::rest_list("Fireflies channel objects", &["id"], &outdoc::SINGLE_PAGE))]
    List,
    /// Get one channel with its members
    #[command(after_long_help = outdoc::fireflies("channel", "channel.id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
pub enum ContactCmd {
    /// List contacts
    #[command(after_long_help = outdoc::rest_list("Fireflies contact objects", &["email"], &outdoc::SINGLE_PAGE))]
    List,
}

#[derive(Subcommand)]
pub enum UserCmd {
    /// List your team's users
    #[command(after_long_help = outdoc::rest_list("Fireflies user objects", &["user_id", "email"], &outdoc::SINGLE_PAGE))]
    List,
    /// Get one user by ID, or `me` for the API key's owner
    #[command(after_long_help = outdoc::fireflies("user", "user.user_id"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = match cmd {
        Cmd::Transcript(cmd) => match cmd {
            TranscriptCmd::List {
                keyword,
                scope,
                from_date,
                to_date,
                organizers,
                participants,
                user,
                channel,
                mine,
                page,
            } => {
                let data = exec::<TranscriptList>(
                    instance,
                    transcript_list::Variables {
                        keyword,
                        scope,
                        from_date,
                        to_date,
                        organizers: non_empty(organizers),
                        participants: non_empty(participants),
                        user_id: user,
                        channel_id: channel,
                        mine: mine.then_some(true),
                        limit: Some(page.limit.into()),
                        skip: Some(page.start_at.into()),
                    },
                )?;
                offset_list(data, "transcripts", &page)
            }
            TranscriptCmd::Get {
                id,
                sentences,
                from,
            } => {
                return pipe::run_get(id, from, format, |id| {
                    exec::<TranscriptGet>(instance, transcript_get::Variables { id, sentences })
                });
            }
            TranscriptCmd::Create { url, title } => exec::<TranscriptCreate>(
                instance,
                transcript_create::Variables {
                    input: Some(transcript_create::AudioUploadInput {
                        url,
                        title,
                        attendees: None,
                        bypass_size_check: None,
                        client_reference_id: None,
                        custom_language: None,
                        download_auth: None,
                        save_video: None,
                        webhook: None,
                    }),
                },
            )?,
            TranscriptCmd::Update { id, title } => exec::<TranscriptUpdate>(
                instance,
                transcript_update::Variables {
                    input: transcript_update::UpdateMeetingTitleInput { id, title },
                },
            )?,
            TranscriptCmd::Delete { id } => {
                exec::<TranscriptDelete>(instance, transcript_delete::Variables { id })?
            }
        },
        Cmd::Askfred(cmd) => match cmd {
            AskfredCmd::List { transcript } => single_page(
                exec::<ThreadList>(
                    instance,
                    thread_list::Variables {
                        transcript_id: transcript,
                    },
                )?,
                "askfred_threads",
            ),
            AskfredCmd::Get { id, from } => {
                return pipe::run_get(id, from, format, |id| {
                    exec::<ThreadGet>(instance, thread_get::Variables { id })
                });
            }
            AskfredCmd::Create {
                question,
                transcript,
                start_time,
                end_time,
                organizers,
                participants,
                channels,
            } => {
                let filters = thread_create::AskFredMeetingFiltersInput {
                    start_time,
                    end_time,
                    organizers: non_empty(organizers),
                    participants: non_empty(participants),
                    channel_ids: non_empty(channels),
                    ..Default::default()
                };
                let any_filter = filters.start_time.is_some()
                    || filters.end_time.is_some()
                    || filters.organizers.is_some()
                    || filters.participants.is_some()
                    || filters.channel_ids.is_some();
                exec::<ThreadCreate>(
                    instance,
                    thread_create::Variables {
                        input: thread_create::CreateAskFredThreadInput {
                            query: question,
                            transcript_id: transcript,
                            filters: any_filter.then_some(filters),
                            ..Default::default()
                        },
                    },
                )?
            }
            AskfredCmd::Continue { id, question } => exec::<ThreadContinue>(
                instance,
                thread_continue::Variables {
                    input: thread_continue::ContinueAskFredThreadInput {
                        thread_id: id,
                        query: question,
                        ..Default::default()
                    },
                },
            )?,
            AskfredCmd::Delete { id } => {
                exec::<ThreadDelete>(instance, thread_delete::Variables { id })?
            }
        },
        Cmd::Bite(cmd) => match cmd {
            BiteCmd::List {
                transcript,
                team,
                page,
            } => {
                // Fireflies needs one scope; with none given, list your own.
                let mine = transcript.is_none() && !team;
                let data = exec::<BiteList>(
                    instance,
                    bite_list::Variables {
                        mine: mine.then_some(true),
                        my_team: team.then_some(true),
                        transcript_id: transcript,
                        limit: Some(page.limit.into()),
                        skip: Some(page.start_at.into()),
                    },
                )?;
                offset_list(data, "bites", &page)
            }
            BiteCmd::Get { id, from } => {
                return pipe::run_get(id, from, format, |id| {
                    exec::<BiteGet>(instance, bite_get::Variables { id })
                });
            }
        },
        Cmd::Channel(cmd) => match cmd {
            ChannelCmd::List => single_page(
                exec::<ChannelList>(instance, channel_list::Variables {})?,
                "channels",
            ),
            ChannelCmd::Get { id, from } => {
                return pipe::run_get(id, from, format, |id| {
                    exec::<ChannelGet>(instance, channel_get::Variables { id })
                });
            }
        },
        Cmd::Contact(ContactCmd::List) => single_page(
            exec::<ContactList>(instance, contact_list::Variables {})?,
            "contacts",
        ),
        Cmd::User(cmd) => match cmd {
            UserCmd::List => single_page(
                exec::<UserList>(instance, user_list::Variables {})?,
                "users",
            ),
            UserCmd::Get { id, from } => {
                return pipe::run_get(id, from, format, |id: String| {
                    // Omitting the ID asks Fireflies for the key's owner.
                    let id = (id != "me").then_some(id);
                    exec::<UserGet>(instance, user_get::Variables { id })
                });
            }
        },
    };
    crate::output::print(&data, format);
    Ok(())
}

fn non_empty(values: Vec<String>) -> Option<Vec<String>> {
    (!values.is_empty()).then_some(values)
}

/// The bare array Fireflies returns under `key`; a null list is empty.
fn take_items(mut data: Value, key: &str) -> Vec<Value> {
    match data[key].take() {
        Value::Array(items) => items,
        _ => Vec::new(),
    }
}

/// Fireflies reports no total, so a full page is the has-more hint.
fn offset_list(data: Value, key: &str, page: &Page) -> Value {
    let items = take_items(data, key);
    let page_info =
        rest::offset_page_info(page.start_at.into(), page.limit, items.len(), None, None);
    rest::wrap_list(items, page_info)
}

fn single_page(data: Value, key: &str) -> Value {
    rest::wrap_list(take_items(data, key), json!({ "hasNextPage": false }))
}

fn exec<Q: GraphQLQuery>(
    instance: &str,
    variables: Q::Variables,
) -> Result<Value, Box<dyn std::error::Error>> {
    let token = crate::auth::fireflies_token(instance)?;
    let body = post::<Q>(API_URL, &token, variables)?;
    if failed(&body) {
        let text = body.to_string();
        if has_code(&body, "object_not_found") {
            return Err(pipe::NotFound(text).into());
        }
        return Err(text.into());
    }
    Ok(data(body))
}

/// POST one operation; the status is ignored because Fireflies signals every
/// failure, even a bad key (HTTP 500, `auth_failed`), in `errors`.
fn post<Q: GraphQLQuery>(
    url: &str,
    token: &str,
    variables: Q::Variables,
) -> Result<Value, reqwest::Error> {
    let response = reqwest::blocking::Client::new()
        .post(url)
        .bearer_auth(token)
        .header("User-Agent", "foac")
        .json(&Q::build_query(variables))
        .send()?;
    let status = response.status();
    let body = rest::parse_response(status, response.text()?);
    Ok(if status.is_success() || body.get("errors").is_some() {
        body
    } else {
        // A non-JSON failure page: surface it as an error all the same.
        json!({ "errors": [body] })
    })
}

fn failed(body: &Value) -> bool {
    body.get("errors").is_some_and(|errors| !errors.is_null())
}

fn data(mut body: Value) -> Value {
    body["data"].take()
}

fn has_code(body: &Value, code: &str) -> bool {
    body["errors"].as_array().is_some_and(|errors| {
        errors
            .iter()
            .any(|error| error["code"] == code || error["extensions"]["code"] == code)
    })
}

pub fn authenticated() -> bool {
    crate::auth::fireflies_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("fireflies")
}

/// The API key owner's `user`, fetched by omitting the ID.
pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    auth_identity_at(API_URL, token)
}

fn auth_identity_at(url: &str, token: &str) -> Result<Value, crate::auth::ValidationError> {
    let body = post::<UserGet>(url, token, user_get::Variables { id: None })
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    if has_code(&body, "auth_failed") {
        return Err(crate::auth::ValidationError::Rejected(body.to_string()));
    }
    if failed(&body) {
        return Err(crate::auth::ValidationError::Failed(body.to_string()));
    }
    Ok(data(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_json(request: &str) -> Value {
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn posts_bearer_auth_and_omits_unset_variables() {
        let (url, request_rx, server) =
            rest::testing::test_server("200 OK", r#"{"data":{"transcripts":[]}}"#, "");
        let body = post::<TranscriptList>(
            url.as_str(),
            "ff-key",
            transcript_list::Variables {
                keyword: Some("roadmap".into()),
                organizers: non_empty(vec!["a@example.com".into()]),
                participants: non_empty(Vec::new()),
                limit: Some(10),
                skip: Some(20),
                ..Default::default()
            },
        )
        .unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST / HTTP/1.1"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer ff-key")
        );
        let sent = request_json(&request);
        assert!(sent["query"].as_str().unwrap().contains("transcripts("));
        assert_eq!(
            sent["variables"],
            json!({
                "keyword": "roadmap",
                "organizers": ["a@example.com"],
                "limit": 10,
                "skip": 20,
            })
        );
        assert_eq!(data(body), json!({ "transcripts": [] }));
    }

    #[test]
    fn transcript_get_includes_sentences_only_on_request() {
        let query = TranscriptGet::build_query(transcript_get::Variables {
            id: "t1".into(),
            sentences: false,
        });
        let sent = serde_json::to_value(&query).unwrap();
        assert_eq!(sent["variables"], json!({ "id": "t1", "sentences": false }));
        assert!(
            sent["query"]
                .as_str()
                .unwrap()
                .contains("sentences @include(if: $sentences)")
        );
    }

    #[test]
    fn askfred_create_sends_filters_only_when_given() {
        let query = ThreadCreate::build_query(thread_create::Variables {
            input: thread_create::CreateAskFredThreadInput {
                query: "What did we decide?".into(),
                transcript_id: Some("t1".into()),
                ..Default::default()
            },
        });
        let sent = serde_json::to_value(&query).unwrap();
        assert_eq!(
            sent["variables"]["input"],
            json!({ "query": "What did we decide?", "transcript_id": "t1" })
        );
    }

    #[test]
    fn lists_wrap_bare_arrays_with_offset_paging() {
        let page = Page {
            limit: 2,
            start_at: 4,
        };
        let full = offset_list(
            json!({ "transcripts": [{"id": "a"}, {"id": "b"}] }),
            "transcripts",
            &page,
        );
        assert_eq!(full["items"], json!([{"id": "a"}, {"id": "b"}]));
        assert_eq!(
            full["pageInfo"],
            json!({ "hasNextPage": true, "nextStartAt": 6 })
        );

        let short = offset_list(
            json!({ "transcripts": [{"id": "a"}] }),
            "transcripts",
            &page,
        );
        assert_eq!(short["pageInfo"]["hasNextPage"], false);

        let null = single_page(json!({ "askfred_threads": null }), "askfred_threads");
        assert_eq!(
            null,
            json!({ "items": [], "pageInfo": { "hasNextPage": false } })
        );
    }

    #[test]
    fn identity_rejects_bad_keys_and_fails_on_other_errors() {
        let ok = r#"{"data":{"user":{"user_id":"u1","email":"a@example.com","name":"A"}}}"#;
        let (url, _, server) = rest::testing::test_server("200 OK", ok, "");
        let identity = auth_identity_at(url.as_str(), "ff-key").unwrap();
        server.join().unwrap();
        assert_eq!(identity["user"]["user_id"], "u1");

        let bad = r#"{"errors":[{"code":"auth_failed","message":"bad key","extensions":{"code":"auth_failed"}}]}"#;
        let (url, _, server) = rest::testing::test_server("500 Internal Server Error", bad, "");
        let error = auth_identity_at(url.as_str(), "bad").unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Rejected(_)));

        let (url, _, server) = rest::testing::test_server("502 Bad Gateway", "upstream down", "");
        let error = auth_identity_at(url.as_str(), "ff-key").unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Failed(_)));
    }

    #[test]
    fn object_not_found_is_detected_in_either_code_field() {
        let body = json!({ "errors": [{ "extensions": { "code": "object_not_found" } }] });
        assert!(has_code(&body, "object_not_found"));
        assert!(!has_code(&json!({ "data": {} }), "object_not_found"));
    }
}
