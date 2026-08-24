//! Shared REST core: the request plumbing every REST provider needs. Slack's
//! HTTP-200 `ok`/error envelope keeps its own `send`; it shares only the
//! helpers and the auth-identity HTTP.

use serde_json::{Map, Value, json};

/// How a provider authenticates requests: a bearer token for most, HTTP
/// Basic (email + API token) for Atlassian.
#[derive(Clone)]
pub(crate) enum Auth {
    Bearer(String),
    Basic { user: String, password: String },
}

impl Auth {
    fn apply(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match self {
            Self::Bearer(token) => request.bearer_auth(token),
            Self::Basic { user, password } => request.basic_auth(user, Some(password)),
        }
    }
}

pub(crate) struct Api {
    pub(crate) client: reqwest::blocking::Client,
    pub(crate) base_url: reqwest::Url,
    pub(crate) auth: Auth,
    pub(crate) format: crate::output::Format,
    /// Provider-specific static headers, e.g. GitHub's Accept and API version.
    pub(crate) headers: &'static [(&'static str, &'static str)],
    /// Sentry requires the trailing slash; without it the API redirects and
    /// some proxies drop the request body.
    pub(crate) trailing_slash: bool,
}

#[derive(Debug)]
pub(crate) struct ApiResponse {
    pub(crate) body: Value,
    pub(crate) link: Option<String>,
}

impl Api {
    pub(crate) fn print(
        &self,
        method: reqwest::Method,
        segments: Vec<String>,
        query: Vec<(&'static str, String)>,
        body: Option<Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let response = self.send(method, &segments, &query, body)?;
        crate::output::print(&response.body, self.format);
        Ok(())
    }

    pub(crate) fn send(
        &self,
        method: reqwest::Method,
        segments: &[String],
        query: &[(&'static str, String)],
        body: Option<Value>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error>> {
        let mut request = self
            .auth
            .apply(self.client.request(method, self.url(segments)?))
            .header("User-Agent", "foac")
            .query(query);
        for (name, value) in self.headers {
            request = request.header(*name, *value);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send()?;
        let status = response.status();
        let link = response
            .headers()
            .get(reqwest::header::LINK)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = parse_response(status, response.text()?);
        if !status.is_success() {
            return Err(body.to_string().into());
        }
        Ok(ApiResponse { body, link })
    }

    fn url(&self, segments: &[String]) -> Result<reqwest::Url, Box<dyn std::error::Error>> {
        let mut url = self.base_url.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| "API base URL cannot be a base")?;
            path.extend(segments);
            if self.trailing_slash {
                path.push("");
            }
        }
        Ok(url)
    }
}

pub(crate) fn wrap_list(items: Vec<Value>, page_info: Value) -> Value {
    json!({
        "items": items,
        "pageInfo": page_info,
    })
}

pub(crate) fn parse_response(status: reqwest::StatusCode, text: String) -> Value {
    if text.is_empty() {
        json!({})
    } else {
        serde_json::from_str(&text)
            .unwrap_or_else(|_| json!({ "status": status.as_u16(), "body": text }))
    }
}

pub(crate) fn push_query<T: ToString>(
    query: &mut Vec<(&'static str, String)>,
    name: &'static str,
    value: Option<T>,
) {
    if let Some(value) = value {
        query.push((name, value.to_string()));
    }
}

/// Mutually exclusive `--body` / `--body-file` flags for Markdown-ish text
/// inputs, shared by the REST providers.
#[derive(clap::Args, Default)]
pub(crate) struct BodyInput {
    /// Markdown body
    #[arg(long, conflicts_with = "body_file")]
    pub(crate) body: Option<String>,
    /// Read the Markdown body from a UTF-8 file
    #[arg(long, conflicts_with = "body")]
    pub(crate) body_file: Option<std::path::PathBuf>,
}

impl BodyInput {
    pub(crate) fn read(self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        match (self.body, self.body_file) {
            (Some(body), None) => Ok(Some(body)),
            (None, Some(path)) => Ok(Some(std::fs::read_to_string(path)?)),
            (None, None) => Ok(None),
            (Some(_), Some(_)) => unreachable!("clap enforces --body xor --body-file"),
        }
    }

    pub(crate) fn required(self) -> Result<String, Box<dyn std::error::Error>> {
        self.read()?
            .ok_or_else(|| "one of --body or --body-file is required".into())
    }
}

pub(crate) fn insert_opt<T: Into<Value>>(
    object: &mut Map<String, Value>,
    name: &str,
    value: Option<T>,
) {
    if let Some(value) = value {
        object.insert(name.to_owned(), value.into());
    }
}

/// Whether user input is an all-digits ID rather than a key or name.
pub(crate) fn is_numeric_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

/// An `id` field as a string, whether the API returned a string or a number.
pub(crate) fn id_string(value: &Value) -> Option<String> {
    match value {
        Value::String(id) => Some(id.clone()),
        Value::Number(id) => Some(id.to_string()),
        _ => None,
    }
}

/// One identity-validation request; providers keep their own status/body
/// interpretation (Slack accepts HTTP 200 with `ok: false` as a rejection).
pub(crate) fn fetch_identity(
    method: reqwest::Method,
    url: reqwest::Url,
    auth: &Auth,
    headers: &'static [(&'static str, &'static str)],
) -> Result<(reqwest::StatusCode, Value), crate::auth::ValidationError> {
    let mut request = auth
        .apply(reqwest::blocking::Client::new().request(method, url))
        .header("User-Agent", "foac");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = request
        .send()
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    let status = response.status();
    let text = response
        .text()
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    Ok((status, parse_response(status, text)))
}

/// GET an identity endpoint; statuses in `rejected` mean a bad token, any
/// other failure is an error.
pub(crate) fn identity(
    url: reqwest::Url,
    auth: &Auth,
    headers: &'static [(&'static str, &'static str)],
    rejected: &[reqwest::StatusCode],
) -> Result<Value, crate::auth::ValidationError> {
    let (status, body) = fetch_identity(reqwest::Method::GET, url, auth, headers)?;
    if rejected.contains(&status) {
        return Err(crate::auth::ValidationError::Rejected(body.to_string()));
    }
    if !status.is_success() {
        return Err(crate::auth::ValidationError::Failed(body.to_string()));
    }
    Ok(body)
}

#[cfg(test)]
pub(crate) mod testing {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// One-shot local HTTP server that captures the raw request text and
    /// serves a canned response. Returns the base URL to point an `Api` at.
    pub(crate) fn test_server(
        status: &str,
        body: &str,
        extra_headers: &str,
    ) -> (
        reqwest::Url,
        mpsc::Receiver<String>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (request_tx, request_rx) = mpsc::channel();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read]);
                let complete = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .is_some_and(|header_end| {
                        let headers = String::from_utf8_lossy(&request[..header_end]);
                        let content_length = headers.lines().find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        });
                        request.len() >= header_end + 4 + content_length.unwrap_or(0)
                    });
                if read == 0 || complete {
                    break;
                }
            }
            let _ = request_tx.send(String::from_utf8(request).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        });
        let url = reqwest::Url::parse(&format!("http://{address}/")).unwrap();
        (url, request_rx, server)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Method;
    use serde_json::json;

    fn test_api(url: reqwest::Url, trailing_slash: bool) -> Api {
        Api {
            client: reqwest::blocking::Client::new(),
            base_url: url,
            auth: Auth::Bearer("secret-token".into()),
            format: crate::output::Format::Json,
            headers: &[("X-Test-Header", "test-value")],
            trailing_slash,
        }
    }

    #[test]
    fn sends_bearer_auth_static_headers_and_parses_json() {
        let (url, request_rx, server) = testing::test_server("200 OK", "{\"ok\":true}", "");
        let api = test_api(url, false);
        let response = api
            .send(
                Method::GET,
                &["resources".into()],
                &[("page", "2".into())],
                None,
            )
            .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /resources?page=2 http/1.1"));
        assert!(request.contains("authorization: bearer secret-token"));
        assert!(request.contains("x-test-header: test-value"));
        assert!(request.contains("user-agent: foac"));
        assert_eq!(response.body, json!({ "ok": true }));
    }

    #[test]
    fn basic_auth_sends_a_basic_authorization_header() {
        let (url, request_rx, server) = testing::test_server("200 OK", "{}", "");
        let api = Api {
            auth: Auth::Basic {
                user: "user@example.com".into(),
                password: "api-token".into(),
            },
            ..test_api(url, false)
        };
        api.send(Method::GET, &["myself".into()], &[], None)
            .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.contains("authorization: basic "));
        assert!(!request.contains("api-token"));
    }

    #[test]
    fn builds_urls_with_an_optional_trailing_slash() {
        let base = reqwest::Url::parse("https://api.example.com").unwrap();
        let api = test_api(base.clone(), true);
        let segments = ["organizations".to_owned(), "acme".to_owned()];
        assert_eq!(
            api.url(&segments).unwrap().as_str(),
            "https://api.example.com/organizations/acme/"
        );
        let api = test_api(base, false);
        assert_eq!(
            api.url(&segments).unwrap().as_str(),
            "https://api.example.com/organizations/acme"
        );
    }

    #[test]
    fn propagates_error_bodies_and_represents_empty_success_as_json() {
        let (url, _, server) =
            testing::test_server("422 Unprocessable Entity", "{\"message\":\"invalid\"}", "");
        let api = test_api(url, false);
        let error = api
            .send(Method::POST, &["resource".into()], &[], Some(json!({})))
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(error.to_string(), "{\"message\":\"invalid\"}");

        let (url, _, server) = testing::test_server("204 No Content", "", "");
        let api = test_api(url, false);
        let response = api
            .send(Method::DELETE, &["resource".into()], &[], None)
            .unwrap();
        server.join().unwrap();
        assert_eq!(response.body, json!({}));
    }

    #[test]
    fn identity_maps_rejected_statuses_to_rejected() {
        let (url, _, server) =
            testing::test_server("401 Unauthorized", "{\"err\":\"bad token\"}", "");
        let error = identity(
            url.join("user").unwrap(),
            &Auth::Bearer("bad".into()),
            &[],
            &[reqwest::StatusCode::UNAUTHORIZED],
        )
        .unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Rejected(_)));

        let (url, _, server) = testing::test_server("500 Internal Server Error", "{}", "");
        let error = identity(
            url.join("user").unwrap(),
            &Auth::Bearer("t".into()),
            &[],
            &[reqwest::StatusCode::UNAUTHORIZED],
        )
        .unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Failed(_)));
    }
}
