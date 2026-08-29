//! Firecrawl provider. REST passthrough against api.firecrawl.dev/v2 with a
//! bearer API key. `scrape`, `map`, and `search` answer synchronously; crawl,
//! batch scrape, and agent are jobs: `create` returns an ID, `get` reads it,
//! and `create --wait` polls until the job settles.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest::{self, Api, Auth, insert_opt, push_query};

pub(crate) const DEFAULT_HOST: &str = "api.firecrawl.dev";
const DEFAULT_URL: &str = "https://api.firecrawl.dev";

#[derive(Args)]
pub struct Cmd {
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Scrape one URL into markdown, HTML, links, a screenshot, a summary,
    /// or structured JSON
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "data": {"markdown": "...", "metadata": {"sourceURL": "...", "statusCode": 200, ...}, ...}}"#,
        "One key per requested format under data (markdown, html, rawHtml, links, images, screenshot, summary, json, ...)",
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Scrape {
        /// URL to scrape
        url: Option<String>,
        #[command(flatten)]
        from: FromFlag,
        #[command(flatten)]
        options: ScrapeOptions,
    },
    /// Discover the URLs of a website
    #[command(after_long_help = outdoc::rest_list("raw Firecrawl link objects (url, title, description)", &["url"], &outdoc::SINGLE_PAGE))]
    Map {
        /// Website URL to map
        url: String,
        /// Rank the discovered URLs by relevance to this query
        #[arg(long)]
        search: Option<String>,
        /// Maximum number of URLs to return
        #[arg(long)]
        limit: Option<u32>,
        /// Sitemap handling: include (default), only, or skip
        #[arg(long)]
        sitemap: Option<String>,
        /// Include URLs on subdomains
        #[arg(long)]
        include_subdomains: bool,
        /// Treat URLs that differ only by query parameters as one
        #[arg(long)]
        ignore_query_parameters: bool,
        /// Timeout in milliseconds
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Search the web, news, or images, optionally scraping each result
    #[command(after_long_help = outdoc::rest_list("raw Firecrawl search results (url, title, description; plus the scraped formats with --scrape-formats)", &["url"], &outdoc::SINGLE_PAGE))]
    Search {
        /// Search query; supports operators like site:, intitle:, and quotes
        query: String,
        /// Maximum number of results per source (Firecrawl default 5, max 100)
        #[arg(long)]
        limit: Option<u32>,
        /// Comma-separated sources: web (default), news, images; results are
        /// concatenated in that order
        #[arg(long, value_delimiter = ',')]
        sources: Vec<String>,
        /// Comma-separated categories to restrict web results to: github,
        /// research, pdf
        #[arg(long, value_delimiter = ',')]
        categories: Vec<String>,
        /// Comma-separated domains to search within
        #[arg(long, value_delimiter = ',')]
        include_domains: Vec<String>,
        /// Comma-separated domains to leave out
        #[arg(long, value_delimiter = ',')]
        exclude_domains: Vec<String>,
        /// Time filter: qdr:h (hour), qdr:d (day), qdr:w (week), qdr:m
        /// (month), qdr:y (year)
        #[arg(long)]
        tbs: Option<String>,
        /// Location to search from, e.g. "Germany" or "San Francisco,California,United States"
        #[arg(long)]
        location: Option<String>,
        /// Drop results whose URL other Firecrawl commands could not scrape
        #[arg(long)]
        ignore_invalid_urls: bool,
        /// Timeout in milliseconds
        #[arg(long)]
        timeout: Option<u64>,
        /// Also scrape every result into these comma-separated formats, e.g.
        /// markdown,links
        #[arg(long, value_delimiter = ',')]
        scrape_formats: Vec<String>,
    },
    /// Crawl jobs: scrape every page reachable from a URL
    #[command(subcommand)]
    Crawl(CrawlCmd),
    /// Batch scrape jobs: scrape many URLs in one job
    #[command(subcommand)]
    BatchScrape(BatchScrapeCmd),
    /// Agent jobs: an AI agent that browses the web to answer a prompt
    #[command(subcommand)]
    Agent(AgentCmd),
    /// Team credits, tokens, queue, and concurrency
    #[command(subcommand)]
    Team(TeamCmd),
}

/// Per-page scrape settings, shared by `scrape`, `crawl create`, and
/// `batch-scrape create`.
#[derive(Args, Default)]
struct ScrapeOptions {
    /// Comma-separated content formats: markdown (default), html, rawHtml,
    /// links, images, screenshot, summary, branding, attributes,
    /// changeTracking
    #[arg(long, value_delimiter = ',')]
    formats: Vec<String>,
    /// Extract structured JSON guided by this prompt (enables the json format)
    #[arg(long)]
    json_prompt: Option<String>,
    /// JSON schema for the structured JSON format, inline (enables the json
    /// format)
    #[arg(long, conflicts_with = "json_schema_file")]
    json_schema: Option<String>,
    /// Read the JSON schema from a file (enables the json format)
    #[arg(long)]
    json_schema_file: Option<PathBuf>,
    /// Capture a full-page screenshot (enables the screenshot format)
    #[arg(long)]
    full_page_screenshot: bool,
    /// Keep only the main content (true, Firecrawl's default) or the whole
    /// page (false)
    #[arg(long)]
    only_main_content: Option<bool>,
    /// Comma-separated HTML tags, classes, or IDs to keep
    #[arg(long, value_delimiter = ',')]
    include_tags: Vec<String>,
    /// Comma-separated HTML tags, classes, or IDs to drop
    #[arg(long, value_delimiter = ',')]
    exclude_tags: Vec<String>,
    /// Extra request headers as a JSON object
    #[arg(long)]
    headers_json: Option<String>,
    /// Milliseconds to wait for the page to settle before scraping
    #[arg(long)]
    wait_for: Option<u64>,
    /// Emulate a mobile device
    #[arg(long)]
    mobile: bool,
    /// Per-page timeout in milliseconds
    #[arg(long)]
    timeout: Option<u64>,
    /// Browser actions to run before scraping (click, scroll, write, ...),
    /// as a JSON array
    #[arg(long)]
    actions_json: Option<String>,
    /// ISO country code to scrape from, e.g. US
    #[arg(long)]
    country: Option<String>,
    /// Comma-separated preferred languages, e.g. en,fr
    #[arg(long, value_delimiter = ',')]
    languages: Vec<String>,
    /// Proxy mode: basic, stealth, or auto
    #[arg(long)]
    proxy: Option<String>,
    /// Accept cached content up to this many milliseconds old
    #[arg(long)]
    max_age: Option<u64>,
}

/// `--wait` and its knobs, shared by every job-creating verb.
#[derive(Args, Default)]
struct Wait {
    /// Poll the job until it completes, fails, or is cancelled, then print
    /// its final status instead of the creation response
    #[arg(long)]
    wait: bool,
    /// Seconds between polls with --wait
    #[arg(long, default_value_t = 5)]
    poll_interval: u64,
    /// Give up after this many seconds with --wait (default: no limit)
    #[arg(long)]
    wait_timeout: Option<u64>,
}

const CRAWL_STATUS: &[&str] = &[
    r#"{"success": true, "status": "scraping|completed|failed|cancelled", "total": 12, "completed": 12, "creditsUsed": 12, "expiresAt": "...", "next": "...", "data": [<page>, ...]}"#,
    "Pages: data[], each a scrape result (markdown, metadata, ...); a `next` URL means more pages: pass its skip value to `get --skip`",
    "Raw Firecrawl data; foac adds no envelope",
];

const JOB_CREATED: &[&str] = &[
    r#"{"success": true, "id": "...", "url": "..."}"#,
    "Primary identifier: id; with --wait, the final job status (see `get --help`) with id added",
    "Raw Firecrawl data; foac adds no envelope",
];

#[derive(Subcommand)]
// One short-lived value on the stack; boxing the create variant isn't worth it.
#[allow(clippy::large_enum_variant)]
enum CrawlCmd {
    /// List the team's active crawl jobs
    #[command(after_long_help = outdoc::rest_list("raw Firecrawl active crawl objects", &["id"], &outdoc::SINGLE_PAGE))]
    List,
    /// Start a crawl
    #[command(after_long_help = outdoc::lines(JOB_CREATED))]
    Create {
        /// URL to start crawling from
        url: String,
        /// Describe the crawl in plain language and let Firecrawl derive the
        /// options
        #[arg(long)]
        prompt: Option<String>,
        /// Maximum number of pages to crawl
        #[arg(long)]
        limit: Option<u32>,
        /// Maximum link depth from the start URL
        #[arg(long)]
        max_depth: Option<u32>,
        /// Comma-separated URL path patterns to crawl
        #[arg(long, value_delimiter = ',')]
        include_paths: Vec<String>,
        /// Comma-separated URL path patterns to skip
        #[arg(long, value_delimiter = ',')]
        exclude_paths: Vec<String>,
        /// Sitemap handling: include (default), only, or skip
        #[arg(long)]
        sitemap: Option<String>,
        /// Treat URLs that differ only by query parameters as one
        #[arg(long)]
        ignore_query_parameters: bool,
        /// Follow links anywhere on the domain, not only below the start URL
        #[arg(long)]
        crawl_entire_domain: bool,
        /// Follow links to other domains
        #[arg(long)]
        allow_external_links: bool,
        /// Follow links to subdomains
        #[arg(long)]
        allow_subdomains: bool,
        /// Milliseconds between requests
        #[arg(long)]
        delay: Option<u64>,
        /// Maximum concurrent page scrapes
        #[arg(long)]
        max_concurrency: Option<u32>,
        /// Webhook URL to notify as the crawl progresses
        #[arg(long)]
        webhook: Option<String>,
        #[command(flatten)]
        scrape: ScrapeOptions,
        #[command(flatten)]
        wait: Wait,
    },
    /// Get a crawl's status and scraped pages
    #[command(after_long_help = outdoc::lines(CRAWL_STATUS))]
    Get {
        id: Option<String>,
        /// Skip this many pages (from the previous response's `next` URL)
        #[arg(long)]
        skip: Option<u32>,
        /// Maximum pages per response
        #[arg(long)]
        limit: Option<u32>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// List the pages a crawl failed on or was blocked from by robots.txt
    #[command(after_long_help = outdoc::lines(&[
        r#"{"errors": [{"id": "...", "timestamp": "...", "url": "...", "error": "..."}, ...], "robotsBlocked": ["<url>", ...]}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Errors { id: String },
    /// Cancel a running crawl
    #[command(after_long_help = outdoc::lines(&[
        r#"{"status": "cancelled"}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Cancel { id: String },
}

#[derive(Subcommand)]
// One short-lived value on the stack; boxing the create variant isn't worth it.
#[allow(clippy::large_enum_variant)]
enum BatchScrapeCmd {
    /// Start a batch scrape of several URLs
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "id": "...", "url": "...", "invalidURLs": []}"#,
        "Primary identifier: id; with --wait, the final job status (see `get --help`) with id added",
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Create {
        /// URLs to scrape
        #[arg(required = true)]
        urls: Vec<String>,
        /// Start the job even if some URLs are invalid, listing them in
        /// invalidURLs
        #[arg(long)]
        ignore_invalid_urls: bool,
        /// Maximum concurrent page scrapes
        #[arg(long)]
        max_concurrency: Option<u32>,
        /// Webhook URL to notify as the job progresses
        #[arg(long)]
        webhook: Option<String>,
        #[command(flatten)]
        scrape: ScrapeOptions,
        #[command(flatten)]
        wait: Wait,
    },
    /// Get a batch scrape's status and scraped pages
    #[command(after_long_help = outdoc::lines(CRAWL_STATUS))]
    Get {
        id: Option<String>,
        /// Skip this many pages (from the previous response's `next` URL)
        #[arg(long)]
        skip: Option<u32>,
        /// Maximum pages per response
        #[arg(long)]
        limit: Option<u32>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// List the URLs a batch scrape failed on
    #[command(after_long_help = outdoc::lines(&[
        r#"{"errors": [{"id": "...", "timestamp": "...", "url": "...", "error": "..."}, ...], "robotsBlocked": ["<url>", ...]}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Errors { id: String },
    /// Cancel a running batch scrape
    #[command(after_long_help = outdoc::lines(&[
        r#"{"status": "cancelled"}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Cancel { id: String },
}

#[derive(Subcommand)]
enum AgentCmd {
    /// Start an agent that browses the web to answer a prompt
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "id": "..."}"#,
        "Primary identifier: id; with --wait, the final job status (see `get --help`) with id added",
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Create {
        /// What to find or extract, in plain language
        prompt: String,
        /// URL to focus on; repeat for several
        #[arg(long = "url")]
        urls: Vec<String>,
        /// Model: spark-1-mini (default, cheaper) or spark-1-pro
        #[arg(long)]
        model: Option<String>,
        /// JSON schema for the structured result, inline
        #[arg(long, conflicts_with = "schema_file")]
        schema: Option<String>,
        /// Read the JSON schema from a file
        #[arg(long)]
        schema_file: Option<PathBuf>,
        /// Fail the job once it has spent this many credits
        #[arg(long)]
        max_credits: Option<u32>,
        /// Webhook URL to notify as the agent progresses
        #[arg(long)]
        webhook: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    /// Get an agent job's status and result
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "status": "processing|completed|failed|cancelled", "data": {...}, "creditsUsed": 21, "expiresAt": "..."}"#,
        "Result: data (present once completed; shaped by --schema when given)",
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Cancel a running agent job
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Cancel { id: String },
}

#[derive(Subcommand)]
enum TeamCmd {
    /// Credits left in the current billing period
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "data": {"remainingCredits": 489590, "planCredits": 500000, "billingPeriodStart": "...", "billingPeriodEnd": "..."}}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    CreditUsage,
    /// Agent tokens left in the current billing period
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "data": {"remainingTokens": 7343850, "planTokens": 7500000, "billingPeriodStart": "...", "billingPeriodEnd": "..."}}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    TokenUsage,
    /// Jobs queued and running for the team
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "jobsInQueue": 0, "activeJobsInQueue": 0, "waitingJobsInQueue": 0, "maxConcurrency": 100, "mostRecentSuccess": "..."}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    QueueStatus,
    /// Concurrent scrapes in flight against the plan's limit
    #[command(after_long_help = outdoc::lines(&[
        r#"{"success": true, "concurrency": 0, "maxConcurrency": 100}"#,
        "Raw Firecrawl data; foac adds no envelope",
    ]))]
    Concurrency,
}

macro_rules! path {
    ($($segment:expr),* $(,)?) => {{
        let mut segments = vec!["v2".to_owned()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let api = api(crate::auth::firecrawl_token(instance)?, format, instance)?;
    match cmd.command {
        Resource::Scrape { url, from, options } => {
            let payload = scrape_options(options)?;
            pipe::run_get(url, from, api.format, |url| {
                let mut payload = payload.clone();
                payload.insert("url".into(), json!(url));
                Ok(api
                    .send(Method::POST, &path!["scrape"], &[], Some(payload.into()))?
                    .body)
            })
        }
        Resource::Map {
            url,
            search,
            limit,
            sitemap,
            include_subdomains,
            ignore_query_parameters,
            timeout,
        } => {
            let mut payload = Map::new();
            payload.insert("url".into(), json!(url));
            insert_opt(&mut payload, "search", search);
            insert_opt(&mut payload, "limit", limit);
            insert_opt(&mut payload, "sitemap", sitemap);
            insert_flag(&mut payload, "includeSubdomains", include_subdomains);
            insert_flag(
                &mut payload,
                "ignoreQueryParameters",
                ignore_query_parameters,
            );
            insert_opt(&mut payload, "timeout", timeout);
            let response = api.send(Method::POST, &path!["map"], &[], Some(payload.into()))?;
            print_items(&api, array_at(&response.body, "links"))
        }
        Resource::Search {
            query,
            limit,
            sources,
            categories,
            include_domains,
            exclude_domains,
            tbs,
            location,
            ignore_invalid_urls,
            timeout,
            scrape_formats,
        } => {
            let mut payload = Map::new();
            payload.insert("query".into(), json!(query));
            insert_opt(&mut payload, "limit", limit);
            insert_list(&mut payload, "sources", sources);
            insert_list(&mut payload, "categories", categories);
            insert_list(&mut payload, "includeDomains", include_domains);
            insert_list(&mut payload, "excludeDomains", exclude_domains);
            insert_opt(&mut payload, "tbs", tbs);
            insert_opt(&mut payload, "location", location);
            insert_flag(&mut payload, "ignoreInvalidURLs", ignore_invalid_urls);
            insert_opt(&mut payload, "timeout", timeout);
            if !scrape_formats.is_empty() {
                payload.insert("scrapeOptions".into(), json!({ "formats": scrape_formats }));
            }
            let response = api.send(Method::POST, &path!["search"], &[], Some(payload.into()))?;
            print_items(&api, search_items(&response.body))
        }
        Resource::Crawl(cmd) => run_crawl(&api, cmd),
        Resource::BatchScrape(cmd) => run_batch_scrape(&api, cmd),
        Resource::Agent(cmd) => run_agent(&api, cmd),
        Resource::Team(cmd) => {
            let segments = match cmd {
                TeamCmd::CreditUsage => path!["team", "credit-usage"],
                TeamCmd::TokenUsage => path!["team", "token-usage"],
                TeamCmd::QueueStatus => path!["team", "queue-status"],
                TeamCmd::Concurrency => path!["concurrency-check"],
            };
            api.print(Method::GET, segments, Vec::new(), None)
        }
    }
}

pub fn authenticated() -> bool {
    crate::auth::firecrawl_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("firecrawl")
}

pub(crate) fn auth_identity(
    token: &str,
    url_override: Option<&str>,
    instance: &str,
) -> Result<Value, crate::auth::ValidationError> {
    let base = match url_override {
        Some(url) => normalize_host(url),
        None => base_url(instance),
    };
    let url = reqwest::Url::parse(&format!("{base}/v2/team/credit-usage"))
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        &[],
        &[reqwest::StatusCode::UNAUTHORIZED],
    )
}

fn run_crawl(api: &Api, cmd: CrawlCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CrawlCmd::List => {
            let response = api.send(Method::GET, &path!["crawl", "active"], &[], None)?;
            print_items(api, array_at(&response.body, "crawls"))
        }
        CrawlCmd::Create {
            url,
            prompt,
            limit,
            max_depth,
            include_paths,
            exclude_paths,
            sitemap,
            ignore_query_parameters,
            crawl_entire_domain,
            allow_external_links,
            allow_subdomains,
            delay,
            max_concurrency,
            webhook,
            scrape,
            wait,
        } => {
            let mut payload = Map::new();
            payload.insert("url".into(), json!(url));
            insert_opt(&mut payload, "prompt", prompt);
            insert_opt(&mut payload, "limit", limit);
            insert_opt(&mut payload, "maxDiscoveryDepth", max_depth);
            insert_list(&mut payload, "includePaths", include_paths);
            insert_list(&mut payload, "excludePaths", exclude_paths);
            insert_opt(&mut payload, "sitemap", sitemap);
            insert_flag(
                &mut payload,
                "ignoreQueryParameters",
                ignore_query_parameters,
            );
            insert_flag(&mut payload, "crawlEntireDomain", crawl_entire_domain);
            insert_flag(&mut payload, "allowExternalLinks", allow_external_links);
            insert_flag(&mut payload, "allowSubdomains", allow_subdomains);
            insert_opt(&mut payload, "delay", delay);
            insert_opt(&mut payload, "maxConcurrency", max_concurrency);
            insert_opt(&mut payload, "webhook", webhook);
            let scrape = scrape_options(scrape)?;
            if !scrape.is_empty() {
                payload.insert("scrapeOptions".into(), scrape.into());
            }
            create_job(api, path!["crawl"], payload, wait)
        }
        CrawlCmd::Get {
            id,
            skip,
            limit,
            from,
        } => job_get(api, path!["crawl"], id, skip, limit, from),
        CrawlCmd::Errors { id } => {
            api.print(Method::GET, path!["crawl", id, "errors"], Vec::new(), None)
        }
        CrawlCmd::Cancel { id } => api.print(Method::DELETE, path!["crawl", id], Vec::new(), None),
    }
}

fn run_batch_scrape(api: &Api, cmd: BatchScrapeCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        BatchScrapeCmd::Create {
            urls,
            ignore_invalid_urls,
            max_concurrency,
            webhook,
            scrape,
            wait,
        } => {
            // Batch scrape takes the per-page options at the top level.
            let mut payload = scrape_options(scrape)?;
            payload.insert("urls".into(), json!(urls));
            insert_flag(&mut payload, "ignoreInvalidURLs", ignore_invalid_urls);
            insert_opt(&mut payload, "maxConcurrency", max_concurrency);
            insert_opt(&mut payload, "webhook", webhook);
            create_job(api, path!["batch", "scrape"], payload, wait)
        }
        BatchScrapeCmd::Get {
            id,
            skip,
            limit,
            from,
        } => job_get(api, path!["batch", "scrape"], id, skip, limit, from),
        BatchScrapeCmd::Errors { id } => api.print(
            Method::GET,
            path!["batch", "scrape", id, "errors"],
            Vec::new(),
            None,
        ),
        BatchScrapeCmd::Cancel { id } => api.print(
            Method::DELETE,
            path!["batch", "scrape", id],
            Vec::new(),
            None,
        ),
    }
}

fn run_agent(api: &Api, cmd: AgentCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        AgentCmd::Create {
            prompt,
            urls,
            model,
            schema,
            schema_file,
            max_credits,
            webhook,
            wait,
        } => {
            let mut payload = Map::new();
            payload.insert("prompt".into(), json!(prompt));
            insert_list(&mut payload, "urls", urls);
            insert_opt(&mut payload, "model", model);
            insert_opt(
                &mut payload,
                "schema",
                read_json(schema, schema_file, "--schema")?,
            );
            insert_opt(&mut payload, "maxCredits", max_credits);
            insert_opt(&mut payload, "webhook", webhook);
            create_job(api, path!["agent"], payload, wait)
        }
        AgentCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!["agent", id], Vec::new())
        }),
        AgentCmd::Cancel { id } => api.print(Method::DELETE, path!["agent", id], Vec::new(), None),
    }
}

fn job_get(
    api: &Api,
    segments: Vec<String>,
    id: Option<String>,
    skip: Option<u32>,
    limit: Option<u32>,
    from: FromFlag,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut query = Vec::new();
    push_query(&mut query, "skip", skip);
    push_query(&mut query, "limit", limit);
    pipe::run_get(id, from, api.format, |id| {
        let mut segments = segments.clone();
        segments.push(id);
        api.get_body(segments, query.clone())
    })
}

/// POST a job, then either print the creation response or, with `--wait`,
/// poll its status until it settles and print that (with the job's `id`
/// added, since status responses omit it).
fn create_job(
    api: &Api,
    segments: Vec<String>,
    payload: Map<String, Value>,
    wait: Wait,
) -> Result<(), Box<dyn std::error::Error>> {
    let created = api
        .send(Method::POST, &segments, &[], Some(payload.into()))?
        .body;
    if !wait.wait {
        crate::output::print(&created, api.format);
        return Ok(());
    }
    let id = created["id"]
        .as_str()
        .ok_or_else(|| format!("Firecrawl did not return a job ID: {created}"))?
        .to_owned();
    let mut job = segments;
    job.push(id.clone());
    let mut body = poll(
        || api.get_body(job.clone(), Vec::new()),
        Duration::from_secs(wait.poll_interval),
        wait.wait_timeout.map(Duration::from_secs),
    )?;
    if let Some(object) = body.as_object_mut() {
        object.entry("id").or_insert_with(|| json!(id));
    }
    crate::output::print(&body, api.format);
    Ok(())
}

/// Fetch a job's status until it is completed, failed, or cancelled.
fn poll(
    mut fetch: impl FnMut() -> Result<Value, Box<dyn std::error::Error>>,
    interval: Duration,
    timeout: Option<Duration>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let started = Instant::now();
    loop {
        let body = fetch()?;
        let status = body["status"].as_str().unwrap_or("unknown");
        if matches!(status, "completed" | "failed" | "cancelled") {
            return Ok(body);
        }
        if timeout.is_some_and(|timeout| started.elapsed() >= timeout) {
            return Err(format!(
                "job is still {status} after {}s; check on it later with `get`",
                started.elapsed().as_secs()
            )
            .into());
        }
        std::thread::sleep(interval);
    }
}

/// The scrape request body from the flags: every omitted flag is left to
/// Firecrawl's defaults.
fn scrape_options(
    options: ScrapeOptions,
) -> Result<Map<String, Value>, Box<dyn std::error::Error>> {
    let mut payload = Map::new();
    let mut formats: Vec<Value> = options
        .formats
        .into_iter()
        .filter(|format| !(options.full_page_screenshot && format == "screenshot"))
        .map(Value::String)
        .collect();
    if options.full_page_screenshot {
        formats.push(json!({ "type": "screenshot", "fullPage": true }));
    }
    let schema = read_json(
        options.json_schema,
        options.json_schema_file,
        "--json-schema",
    )?;
    if options.json_prompt.is_some() || schema.is_some() {
        let mut format = Map::new();
        format.insert("type".into(), json!("json"));
        insert_opt(&mut format, "prompt", options.json_prompt);
        insert_opt(&mut format, "schema", schema);
        formats.push(format.into());
    }
    if !formats.is_empty() {
        payload.insert("formats".into(), formats.into());
    }
    insert_opt(&mut payload, "onlyMainContent", options.only_main_content);
    insert_list(&mut payload, "includeTags", options.include_tags);
    insert_list(&mut payload, "excludeTags", options.exclude_tags);
    insert_opt(
        &mut payload,
        "headers",
        parse_json(options.headers_json, "--headers-json")?,
    );
    insert_opt(&mut payload, "waitFor", options.wait_for);
    insert_flag(&mut payload, "mobile", options.mobile);
    insert_opt(&mut payload, "timeout", options.timeout);
    insert_opt(
        &mut payload,
        "actions",
        parse_json(options.actions_json, "--actions-json")?,
    );
    if options.country.is_some() || !options.languages.is_empty() {
        let mut location = Map::new();
        insert_opt(&mut location, "country", options.country);
        insert_list(&mut location, "languages", options.languages);
        payload.insert("location".into(), location.into());
    }
    insert_opt(&mut payload, "proxy", options.proxy);
    insert_opt(&mut payload, "maxAge", options.max_age);
    Ok(payload)
}

fn parse_json(
    text: Option<String>,
    flag: &str,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    text.map(|text| {
        serde_json::from_str(&text)
            .map_err(|error| format!("{flag} is not valid JSON: {error}").into())
    })
    .transpose()
}

fn read_json(
    inline: Option<String>,
    file: Option<PathBuf>,
    flag: &str,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let text = match file {
        Some(path) => Some(std::fs::read_to_string(path)?),
        None => inline,
    };
    parse_json(text, flag)
}

fn insert_flag(object: &mut Map<String, Value>, name: &str, flag: bool) {
    if flag {
        object.insert(name.to_owned(), json!(true));
    }
}

fn insert_list(object: &mut Map<String, Value>, name: &str, values: Vec<String>) {
    if !values.is_empty() {
        object.insert(name.to_owned(), json!(values));
    }
}

fn print_items(api: &Api, items: Vec<Value>) -> Result<(), Box<dyn std::error::Error>> {
    crate::output::print(
        &rest::wrap_list(items, json!({ "hasNextPage": false })),
        api.format,
    );
    Ok(())
}

fn array_at(body: &Value, key: &str) -> Vec<Value> {
    body[key].as_array().cloned().unwrap_or_default()
}

/// Search results are grouped by source under `data`; flatten them into one
/// list, web first, so the output is a foac list like any other.
fn search_items(body: &Value) -> Vec<Value> {
    if let Some(items) = body["data"].as_array() {
        return items.clone();
    }
    ["web", "news", "images"]
        .iter()
        .flat_map(|source| array_at(&body["data"], source))
        .collect()
}

fn api(
    token: String,
    format: crate::output::Format,
    instance: &str,
) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(&base_url(instance))?,
        auth: Auth::Bearer(token),
        format,
        headers: &[],
        trailing_slash: false,
    })
}

/// The instance's base URL: `FIRECRAWL_API_URL` (default instance only),
/// then the URL stored with the instance's credentials, then the cloud API.
pub(crate) fn base_url(instance: &str) -> String {
    let environment = if instance == crate::provider::DEFAULT_INSTANCE {
        std::env::var("FIRECRAWL_API_URL").ok()
    } else {
        None
    };
    resolve_base_url(
        environment,
        crate::auth::stored_url(crate::provider::Credential::FirecrawlUrl, instance),
    )
}

/// Turn the host the user gave at login into a base URL: empty means the
/// cloud API, a bare host gets https, and an explicit scheme is kept so a
/// local Docker deployment can stay on http.
pub(crate) fn normalize_host(input: &str) -> String {
    let host = input.trim().trim_end_matches('/');
    if host.is_empty() {
        DEFAULT_URL.to_owned()
    } else if host.contains("://") {
        host.to_owned()
    } else {
        format!("https://{host}")
    }
}

fn resolve_base_url(environment: Option<String>, stored: Option<String>) -> String {
    environment
        .filter(|url| !url.trim().is_empty())
        .or(stored)
        .map(|url| normalize_host(&url))
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn scrape_options_build_the_firecrawl_body() {
        assert!(scrape_options(ScrapeOptions::default()).unwrap().is_empty());

        let options = ScrapeOptions {
            formats: vec!["markdown".into(), "screenshot".into(), "links".into()],
            json_prompt: Some("Extract the price".into()),
            json_schema: Some(r#"{"type":"object"}"#.into()),
            full_page_screenshot: true,
            only_main_content: Some(false),
            include_tags: vec!["article".into()],
            wait_for: Some(500),
            mobile: true,
            country: Some("DE".into()),
            languages: vec!["de".into(), "en".into()],
            headers_json: Some(r#"{"X-Test":"1"}"#.into()),
            ..Default::default()
        };
        assert_eq!(
            Value::Object(scrape_options(options).unwrap()),
            json!({
                "formats": [
                    "markdown",
                    "links",
                    { "type": "screenshot", "fullPage": true },
                    { "type": "json", "prompt": "Extract the price", "schema": { "type": "object" } },
                ],
                "onlyMainContent": false,
                "includeTags": ["article"],
                "headers": { "X-Test": "1" },
                "waitFor": 500,
                "mobile": true,
                "location": { "country": "DE", "languages": ["de", "en"] },
            })
        );

        let error = scrape_options(ScrapeOptions {
            actions_json: Some("not json".into()),
            ..Default::default()
        })
        .unwrap_err();
        assert!(error.to_string().contains("--actions-json"));
    }

    #[test]
    fn search_items_flatten_sources_web_first() {
        let body = json!({
            "success": true,
            "data": {
                "news": [{ "url": "n" }],
                "web": [{ "url": "w1" }, { "url": "w2" }],
            },
        });
        assert_eq!(
            search_items(&body),
            vec![
                json!({ "url": "w1" }),
                json!({ "url": "w2" }),
                json!({ "url": "n" })
            ]
        );
        assert_eq!(
            search_items(&json!({ "data": [{ "url": "a" }] })),
            vec![json!({ "url": "a" })]
        );
        assert!(search_items(&json!({})).is_empty());
    }

    #[test]
    fn poll_stops_on_a_final_status_or_the_timeout() {
        let statuses = std::cell::RefCell::new(vec!["completed", "scraping", "scraping"]);
        let body = poll(
            || Ok(json!({ "status": statuses.borrow_mut().pop().unwrap() })),
            Duration::ZERO,
            None,
        )
        .unwrap();
        assert_eq!(body["status"], "completed");
        assert!(statuses.borrow().is_empty());

        let error = poll(
            || Ok(json!({ "status": "processing" })),
            Duration::ZERO,
            Some(Duration::ZERO),
        )
        .unwrap_err();
        assert!(error.to_string().contains("still processing"));

        assert!(poll(|| Err("boom".into()), Duration::ZERO, None).is_err());
    }

    #[test]
    fn normalizes_login_hosts_keeping_an_explicit_scheme() {
        assert_eq!(normalize_host("\n"), DEFAULT_URL);
        assert_eq!(
            normalize_host(" firecrawl.example.com/ \n"),
            "https://firecrawl.example.com"
        );
        assert_eq!(
            normalize_host("http://localhost:3002/"),
            "http://localhost:3002"
        );
    }

    #[test]
    fn base_url_prefers_environment_then_stored_then_default() {
        assert_eq!(
            resolve_base_url(Some("http://env:3002/".into()), Some("x".into())),
            "http://env:3002"
        );
        assert_eq!(
            resolve_base_url(Some(" ".into()), Some("stored.example.com".into())),
            "https://stored.example.com"
        );
        assert_eq!(resolve_base_url(None, None), DEFAULT_URL);
    }

    #[test]
    fn crawl_create_posts_the_crawl_body() {
        let (api, request_rx, server) = test_api("200 OK", r#"{"success":true,"id":"j1"}"#);
        run_crawl(
            &api,
            CrawlCmd::Create {
                url: "https://example.com".into(),
                prompt: None,
                limit: Some(10),
                max_depth: Some(2),
                include_paths: vec!["/docs/*".into()],
                exclude_paths: Vec::new(),
                sitemap: Some("skip".into()),
                ignore_query_parameters: true,
                crawl_entire_domain: false,
                allow_external_links: false,
                allow_subdomains: false,
                delay: None,
                max_concurrency: None,
                webhook: None,
                scrape: ScrapeOptions {
                    formats: vec!["markdown".into()],
                    ..Default::default()
                },
                wait: Wait::default(),
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v2/crawl "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer ")
        );
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({
                "url": "https://example.com",
                "limit": 10,
                "maxDiscoveryDepth": 2,
                "includePaths": ["/docs/*"],
                "sitemap": "skip",
                "ignoreQueryParameters": true,
                "scrapeOptions": { "formats": ["markdown"] },
            })
        );
    }

    #[test]
    fn batch_scrape_create_puts_scrape_options_at_the_top_level() {
        let (api, request_rx, server) = test_api("200 OK", r#"{"success":true,"id":"j1"}"#);
        run_batch_scrape(
            &api,
            BatchScrapeCmd::Create {
                urls: vec!["https://a.example".into(), "https://b.example".into()],
                ignore_invalid_urls: true,
                max_concurrency: Some(2),
                webhook: None,
                scrape: ScrapeOptions {
                    formats: vec!["links".into()],
                    ..Default::default()
                },
                wait: Wait::default(),
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v2/batch/scrape "));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({
                "urls": ["https://a.example", "https://b.example"],
                "ignoreInvalidURLs": true,
                "maxConcurrency": 2,
                "formats": ["links"],
            })
        );
    }

    #[test]
    fn crawl_get_sends_skip_and_limit() {
        let (api, request_rx, server) = test_api("200 OK", r#"{"status":"completed"}"#);
        run_crawl(
            &api,
            CrawlCmd::Get {
                id: Some("j1".into()),
                skip: Some(20),
                limit: Some(10),
                from: FromFlag::default(),
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("GET /v2/crawl/j1?skip=20&limit=10 "));
    }

    #[test]
    fn crawl_cancel_deletes_the_job() {
        let (api, request_rx, server) = test_api("200 OK", r#"{"status":"cancelled"}"#);
        run_crawl(&api, CrawlCmd::Cancel { id: "j1".into() }).unwrap();
        server.join().unwrap();
        assert!(
            request_rx
                .recv()
                .unwrap()
                .starts_with("DELETE /v2/crawl/j1 ")
        );
    }

    #[test]
    fn agent_create_sends_prompt_urls_and_schema() {
        let (api, request_rx, server) = test_api("200 OK", r#"{"success":true,"id":"a1"}"#);
        run_agent(
            &api,
            AgentCmd::Create {
                prompt: "Find the pricing".into(),
                urls: vec!["https://example.com".into()],
                model: Some("spark-1-pro".into()),
                schema: Some(r#"{"type":"object"}"#.into()),
                schema_file: None,
                max_credits: Some(50),
                webhook: None,
                wait: Wait::default(),
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("POST /v2/agent "));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({
                "prompt": "Find the pricing",
                "urls": ["https://example.com"],
                "model": "spark-1-pro",
                "schema": { "type": "object" },
                "maxCredits": 50,
            })
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
            auth: Auth::Bearer("fc-secret".into()),
            headers: &[],
            trailing_slash: false,
        };
        (api, request_rx, server)
    }
}
