//! Shared success-output printer for provider commands: compact
//! JSON for machines, tables sized to the terminal for humans.

use std::borrow::Cow;
use std::io::IsTerminal;

use serde_json::{Map, Value};
use tabled::builder::Builder;
use tabled::grid::util::string::{get_char_width, get_line_width};
use tabled::settings::object::Rows;
use tabled::settings::peaker::Priority;
use tabled::settings::{Color, Style, Width};

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum FormatArg {
    Auto,
    Json,
    Table,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Json,
    Table,
}

/// Pure format resolution: flag beats `FOAC_FORMAT`, which beats `CI`, which
/// beats the stdout TTY check. Unknown env values are ignored.
///
/// ```
/// use foac::output::{Format, FormatArg, resolve};
///
/// assert_eq!(resolve(FormatArg::Json, Some("table"), false, true), Format::Json);
/// assert_eq!(resolve(FormatArg::Auto, Some("table"), true, false), Format::Table);
/// assert_eq!(resolve(FormatArg::Auto, Some("bogus"), false, true), Format::Table);
/// assert_eq!(resolve(FormatArg::Auto, None, true, true), Format::Json);
/// assert_eq!(resolve(FormatArg::Auto, None, false, false), Format::Json);
/// ```
pub fn resolve(flag: FormatArg, env: Option<&str>, ci: bool, tty: bool) -> Format {
    match flag {
        FormatArg::Json => return Format::Json,
        FormatArg::Table => return Format::Table,
        FormatArg::Auto => {}
    }
    if let Some(value) = env.map(str::trim) {
        if value.eq_ignore_ascii_case("json") {
            return Format::Json;
        }
        if value.eq_ignore_ascii_case("table") {
            return Format::Table;
        }
    }
    if ci || !tty {
        Format::Json
    } else {
        Format::Table
    }
}

pub fn print(value: &Value, format: Format) {
    emit(value, format, None);
}

/// Like [`print`], but the table bolds every value cell of `highlight_key`.
/// JSON is unchanged; ANSI is omitted when stdout is not a TTY or `NO_COLOR`
/// is set to a non-empty value.
pub fn print_highlighting(value: &Value, format: Format, highlight_key: &str) {
    emit(value, format, Some(highlight_key));
}

/// One combined table for a pipe-mode `get` run: one row per response.
/// Only table mode reaches here; JSON pipe output streams one document per
/// get instead.
pub fn print_get_rows(bodies: &[Value]) {
    let rows: Vec<Value> = bodies.iter().map(get_row).collect();
    print(
        &serde_json::json!({ "items": rows, "pageInfo": { "count": rows.len() } }),
        Format::Table,
    );
}

/// A get response as a table row. A known envelope — a one-key wrapper
/// (Linear's `{user}`), Slack's `ok` beside the resource, or Jira's
/// `fields` — flattens into the wrapper's scalars plus the inner object's
/// fields, so the row shows the resource, not the envelope. A flat resource
/// whose one nested value is incidental (Jira user's `avatarUrls`) is left
/// alone.
fn get_row(body: &Value) -> Value {
    let Some(wrapper) = body.as_object() else {
        return body.clone();
    };
    let mut nested = wrapper
        .iter()
        .filter(|(_, value)| value.is_object() || value.is_array());
    match (nested.next(), nested.next()) {
        (Some((key, Value::Object(inner))), None)
            if wrapper.len() == 1 || wrapper.contains_key("ok") || key == "fields" =>
        {
            let mut row: Map<String, Value> = wrapper
                .iter()
                .filter(|(_, value)| !value.is_object())
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            for (key, value) in inner {
                row.entry(key.clone()).or_insert_with(|| value.clone());
            }
            Value::Object(row)
        }
        _ => body.clone(),
    }
}

/// JSON uses [`print`]; table mode writes `text` as-is (auth summaries).
pub fn print_text(text: &str, value: &Value, format: Format) {
    match format {
        Format::Json => print(value, format),
        Format::Table => print!("{text}"),
    }
}

/// How a table is drawn: the terminal width, and whether ANSI styling (bold
/// headers, dimmed footers, the highlighted row) is on.
#[derive(Clone, Copy)]
struct Look<'a> {
    width: usize,
    color: bool,
    /// Keyed tables bold every value cell of this row.
    highlight_key: Option<&'a str>,
}

fn emit(value: &Value, format: Format, highlight_key: Option<&str>) {
    let look = Look {
        width: terminal_size::terminal_size().map_or(80, |(w, _)| w.0 as usize),
        color: color_enabled(),
        highlight_key,
    };
    print!("{}", render_inner(value, format, look));
}

pub fn color_enabled() -> bool {
    color_on(std::io::stdout().is_terminal())
}

/// ANSI is omitted off a terminal or when `NO_COLOR` is set to a non-empty
/// value.
fn color_on(terminal: bool) -> bool {
    terminal && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
}

/// Print a failed command's error on stderr. JSON mode prints it untouched,
/// so a provider's error body stays parseable. A human gets an `error:`
/// headline; when the error is a provider's JSON body, the headline carries
/// its message and the body follows, so no detail is lost.
pub fn print_error(error: &dyn std::error::Error, format: Format) {
    let text = error.to_string();
    match format {
        Format::Json => eprintln!("{text}"),
        Format::Table => {
            let color = color_on(std::io::stderr().is_terminal());
            eprint!("{}", render_error(&text, color));
        }
    }
}

fn render_error(text: &str, color: bool) -> String {
    let (red, dim, reset) = if color {
        ("\x1b[1;31m", "\x1b[2m", "\x1b[0m")
    } else {
        ("", "", "")
    };
    match error_headline(text) {
        Some(headline) => format!("{red}error:{reset} {headline}\n{dim}{text}{reset}\n"),
        None => format!("{red}error:{reset} {text}\n"),
    }
}

/// The human-readable message inside a provider's JSON error body, or `None`
/// when `text` is not one. A heuristic over the shapes the providers return:
/// `message` (GitHub, Notion, Neon, Axiom), `error` as a string (Slack,
/// Firecrawl) or an object (Vercel, Airtable), `errorMessages` and an
/// `errors` map (Jira), an `errors` array (Linear, Fireflies, Confluence),
/// and `detail` (Sentry).
pub fn error_headline(text: &str) -> Option<String> {
    let body: Value = serde_json::from_str(text).ok()?;
    let body = body.as_object()?;
    let text = |value: Option<&Value>| {
        value
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let first = |key: &str| body.get(key).and_then(Value::as_array)?.first();
    text(body.get("message"))
        .or_else(|| text(body.get("error")))
        .or_else(|| text(body.get("error")?.get("message")))
        .or_else(|| text(first("errorMessages")))
        .or_else(|| {
            let error = first("errors")?;
            text(Some(error)).or_else(|| {
                ["message", "title", "detail"]
                    .iter()
                    .find_map(|key| text(error.get(key)))
            })
        })
        .or_else(|| {
            let (field, message) = body.get("errors")?.as_object()?.iter().next()?;
            Some(format!("{field}: {}", message.as_str()?))
        })
        .or_else(|| text(body.get("detail")))
        .or_else(|| text(body.get("detail")?.get("message")))
}

#[cfg(test)]
fn render(value: &Value, format: Format, width: usize) -> String {
    let look = Look {
        width,
        color: false,
        highlight_key: None,
    };
    render_inner(value, format, look)
}

fn render_inner(value: &Value, format: Format, look: Look) -> String {
    if format == Format::Json {
        return format!("{value}\n");
    }
    let Some(root) = value.as_object() else {
        return format!("{value}\n");
    };
    // GitHub list: {items, pageInfo} from the REST Link header.
    if let (Some(Value::Array(items)), Some(Value::Object(page))) =
        (root.get("items"), root.get("pageInfo"))
    {
        return render_list(items, page, look);
    }
    if root.len() == 1 {
        let inner = root.values().next().unwrap();
        if let Some(inner) = inner.as_object() {
            // Depth-1 Linear list: the wrapper value is the connection.
            if let Some((nodes, page)) = connection(inner) {
                return render_list(nodes, page, look);
            }
            // Depth-2 Linear list (CommentList, CycleList): a field of the
            // wrapper is the connection; sibling fields are metadata.
            if let Some((nodes, page)) = inner
                .values()
                .find_map(|f| f.as_object().and_then(connection))
            {
                return render_list(nodes, page, look);
            }
            // Single object behind a one-key wrapper (gets, mutations).
            return render_object(inner, look);
        }
    }
    // Keyed map (provider list, auth status): one row per key.
    if root.len() > 1 && root.values().all(Value::is_object) {
        return render_keyed(root, look);
    }
    render_object(root, look)
}

/// `nodes` only counts as a list when `pageInfo` sits next to it; bare
/// `nodes` (issue labels, team members) are relation fields.
fn connection(obj: &Map<String, Value>) -> Option<(&Vec<Value>, &Map<String, Value>)> {
    match (obj.get("nodes"), obj.get("pageInfo")) {
        (Some(Value::Array(nodes)), Some(Value::Object(page))) => Some((nodes, page)),
        _ => None,
    }
}

/// Where a list column or an object row sorts: what a human looks for
/// first. API order is kept within a tier, except `Handle` and `Core`,
/// which are ranked: Jira returns an issue's fields in no useful order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Tier {
    /// The record's own ID when it is how the record is named: a short ID
    /// that commands take, or the only thing naming the row.
    LeadId,
    /// What a human calls the record (`identifier`, `number`, `name`, ...),
    /// carrying its rank in `HANDLE_KEYS`.
    Handle(usize),
    /// What a human triages by (state, priority, who has it), carrying its
    /// rank in `CORE_KEYS`.
    Core(usize),
    Plain,
    Timestamp,
    /// Opaque references: `id`, `node_id`, `accountId`.
    Id,
    Url,
    /// Unreadable without a lookup: Jira's `customfield_10042` fields and
    /// its `expand` hint.
    Opaque,
}

/// In display order, compared ignoring case: Airtable field names are the
/// user's (`Name`, `Status`).
const HANDLE_KEYS: [&str; 10] = [
    "identifier",
    "number",
    "key",
    "shortId",
    "name",
    "title",
    "summary",
    "slug",
    "login",
    "username",
];

/// In display order, compared ignoring case.
const CORE_KEYS: [&str; 15] = [
    "state",
    "status",
    "priority",
    "assignee",
    "assignees",
    "issuetype",
    "type",
    "labels",
    "project",
    "team",
    "owner",
    "user",
    "author",
    "reporter",
    "creator",
];

/// A list column: its header, one cell per row, and where it sorts.
struct Column<'a> {
    key: &'a str,
    cells: Vec<String>,
    tier: Tier,
}

impl Column<'_> {
    /// The widest cell or the header, in terminal columns (an emoji or a
    /// CJK glyph takes two), measured the way `tabled` draws it.
    fn natural(&self) -> usize {
        self.cells
            .iter()
            .map(|cell| get_line_width(cell))
            .chain([get_line_width(self.key)])
            .max()
            .unwrap_or(0)
    }

    /// The width a column is charged before later columns are dropped. Text
    /// is capped so one long column cannot evict everything after it; an ID
    /// is copied rather than read, so a UUID is shown whole or not at all.
    fn budget(&self) -> usize {
        let cap = if self.tier == Tier::Id || is_primary_id(self.key) {
            40
        } else {
            30
        };
        self.natural().min(cap)
    }
}

fn rank(keys: &[&str], key: &str) -> Option<usize> {
    keys.iter()
        .position(|ranked| ranked.eq_ignore_ascii_case(key))
}

/// Whether a field names its record: a handle key, or a Notion page's
/// `title` property, which sits under whatever name the user gave it.
fn is_handle(key: &str, value: &Value) -> bool {
    rank(&HANDLE_KEYS, key).is_some() || is_notion_title(value)
}

fn is_notion_title(value: &Value) -> bool {
    value["type"] == "title" && value["title"].is_array()
}

fn is_primary_id(key: &str) -> bool {
    matches!(key, "id" | "uid")
}

/// `has_handle`: whether the record has a handle field. Without one the
/// primary ID is the only way to name a row, so it leads.
fn tier<'a>(key: &str, values: impl Iterator<Item = &'a Value> + Clone, has_handle: bool) -> Tier {
    if let Some(rank) = rank(&HANDLE_KEYS, key) {
        return Tier::Handle(rank);
    }
    if values.clone().any(is_notion_title) {
        return Tier::Handle(rank(&HANDLE_KEYS, "title").unwrap_or(0));
    }
    if let Some(rank) = rank(&CORE_KEYS, key) {
        return Tier::Core(rank);
    }
    if key == "expand" || key.starts_with("customfield_") {
        return Tier::Opaque;
    }
    // What the values read as, blanks aside.
    let texts: Vec<String> = values
        .filter_map(summary)
        .filter(|text| !text.is_empty())
        .collect();
    let all = |test: fn(&str) -> bool| !texts.is_empty() && texts.iter().all(|text| test(text));
    if is_primary_id(key) {
        // A short ID with letters in it (Slack's C..., Neon's br-...,
        // Airtable's rec...) is what commands take; a UUID or a database
        // number (GitHub's, Jira's "10042") is only ever copied.
        let named = texts.iter().all(|text| {
            text.chars().count() <= 24 && !text.bytes().all(|byte| byte.is_ascii_digit())
        });
        return if named || !has_handle {
            Tier::LeadId
        } else {
            Tier::Id
        };
    }
    if key.ends_with("_id") || key.ends_with("Id") || key.ends_with("ID") || all(is_uuid) {
        Tier::Id
    } else if all(is_url) {
        Tier::Url
    } else if all(is_timestamp) {
        Tier::Timestamp
    } else {
        Tier::Plain
    }
}

fn is_url(text: &str) -> bool {
    text.starts_with("https://") || text.starts_with("http://")
}

/// An ISO 8601 date or date-time: `2026-11-15` or `2026-11-15T00:17:30Z`.
fn is_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 10
        && bytes[..10].iter().enumerate().all(|(i, byte)| match i {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
        && bytes.get(10).is_none_or(|byte| matches!(byte, b'T' | b' '))
}

/// `b439481e-60b2-4da2-abcf-28707c7dff22`.
fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(i, byte)| match i {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// Some providers keep a record's data one level down: Jira issues and
/// Airtable records under `fields`, Notion pages under `properties`. Lift it
/// so the table shows the record, not its envelope: the record's data
/// first, then the envelope's own keys, which win a name collision.
fn spread_fields(record: &Map<String, Value>) -> Cow<'_, Map<String, Value>> {
    let (envelope, fields) = match (record.get("fields"), record.get("properties")) {
        (Some(Value::Object(fields)), _) => ("fields", fields),
        (_, Some(Value::Object(properties))) if is_notion_properties(properties) => {
            ("properties", properties)
        }
        _ => return Cow::Borrowed(record),
    };
    let entry = |(key, value): (&String, &Value)| (key.clone(), value.clone());
    let mut spread: Map<String, Value> = fields
        .iter()
        .filter(|(key, _)| !record.contains_key(*key))
        .map(entry)
        .collect();
    spread.extend(record.iter().filter(|(key, _)| *key != envelope).map(entry));
    Cow::Owned(spread)
}

/// A Notion page's `properties`: every value is tagged with its `type` and
/// carries no `name` of its own. A data source's schema entries do carry
/// one; those stay nested. So does Slack's untagged `properties`.
fn is_notion_properties(properties: &Map<String, Value>) -> bool {
    properties.values().all(|property| {
        property["type"]
            .as_str()
            .is_some_and(|kind| property.get(kind).is_some())
            && property.get("name").is_none()
    })
}

fn render_list(rows: &[Value], page: &Map<String, Value>, look: Look) -> String {
    if rows.is_empty() {
        return "No results.\n".to_owned();
    }
    // A row that is not an object (a bare string) is its own one-cell row.
    let rows: Vec<Cow<Map<String, Value>>> = rows
        .iter()
        .map(|row| match row {
            Value::Object(record) => spread_fields(record),
            other => Cow::Owned(Map::from_iter([("value".to_owned(), other.clone())])),
        })
        .collect();
    // Union of keys across rows, in first-seen order.
    let mut keys: Vec<&str> = Vec::new();
    for row in &rows {
        for key in row.keys() {
            if !keys.contains(&key.as_str()) {
                keys.push(key);
            }
        }
    }
    let has_handle = keys.iter().any(|key| {
        rows.iter()
            .any(|row| is_handle(key, row.get(*key).unwrap_or(&Value::Null)))
    });
    // A column needs data in some row and a short form in every row: nested
    // values with nothing naming them are left to --format json.
    let mut with_data = 0;
    let mut columns: Vec<Column> = Vec::new();
    for key in keys {
        let values = || rows.iter().map(|row| row.get(key).unwrap_or(&Value::Null));
        if values().all(|value| summary(value).is_some_and(|text| text.is_empty())) {
            continue;
        }
        with_data += 1;
        let cells: Option<Vec<String>> = values()
            .map(|value| summary(value).map(|text| one_line(&text)))
            .collect();
        if let Some(cells) = cells {
            let tier = tier(key, values(), has_handle);
            columns.push(Column { key, cells, tier });
        }
    }
    // REST rows do not reliably lead with what identifies them (GitHub
    // issues open with six URLs), so order by tier before cutting to fit.
    columns.sort_by_key(|column| column.tier);
    // Keep only the leading columns that fit the terminal: REST rows carry
    // ~80 fields, and rendering them all crushes every cell to "…".
    let kept = fit_columns(&columns, look.width);
    let columns = &columns[..kept];
    let widths = column_widths(columns, look.width);

    let mut footer = count(rows.len(), "row");
    let hidden = with_data - kept;
    if hidden > 0 {
        footer.push_str(&format!(
            ", {} (--format json shows all)",
            count(hidden, "more column")
        ));
    }
    if page.get("hasNextPage") == Some(&Value::Bool(true)) {
        let cursors: Vec<String> = page
            .iter()
            .filter(|(key, value)| {
                !key.starts_with("has") && !key.starts_with("previous") && !value.is_null()
            })
            .map(|(key, value)| format!("{key}={}", cell(value)))
            .collect();
        let cursors = match cursors.is_empty() {
            true => String::new(),
            false => format!(": {}", cursors.join(" ")),
        };
        footer.push_str(&format!(
            "\nMore results{cursors} (see --help for the paging flag)"
        ));
    }
    let footer = dim(&footer, look);
    if columns.is_empty() {
        return format!("{footer}\n");
    }
    let mut builder = Builder::default();
    builder.push_record(columns.iter().map(|column| column.key));
    for row in 0..rows.len() {
        builder.push_record(
            columns
                .iter()
                .zip(&widths)
                .map(|(column, width)| clip(&column.cells[row], *width)),
        );
    }
    format!("{}\n\n{footer}\n", table(builder, look, &[]))
}

/// A footer under a table, dimmed line by line when color is on.
fn dim(footer: &str, look: Look) -> String {
    if !look.color {
        return footer.to_owned();
    }
    footer
        .lines()
        .map(|line| format!("\x1b[2m{line}\x1b[22m"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `1 row`, `3 rows`.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// How many leading columns fit in `width`, each costing its budget plus the
/// " | " separator. Keeps at least one when there is one.
fn fit_columns(columns: &[Column], width: usize) -> usize {
    let mut remaining = width;
    let mut kept = 0;
    for (i, column) in columns.iter().enumerate() {
        let cost = column.budget() + 3;
        if i > 0 && cost > remaining {
            break;
        }
        remaining = remaining.saturating_sub(cost);
        kept = i + 1;
    }
    kept
}

/// Each kept column's budget, then whatever width is left goes to the
/// columns it cut short, first column first.
fn column_widths(columns: &[Column], width: usize) -> Vec<usize> {
    let mut widths: Vec<usize> = columns.iter().map(Column::budget).collect();
    let used: usize = widths.iter().map(|width| width + 3).sum();
    let mut spare = width.saturating_sub(used);
    for (width, column) in widths.iter_mut().zip(columns) {
        let extra = (column.natural() - *width).min(spare);
        *width += extra;
        spare -= extra;
    }
    widths
}

/// `text` cut to `width` terminal columns, the last one an ellipsis when cut.
fn clip(text: &str, width: usize) -> String {
    if get_line_width(text) <= width {
        return text.to_owned();
    }
    let mut clipped = String::new();
    // One column is the ellipsis's.
    let mut used = 1;
    for c in text.chars() {
        used += get_char_width(c);
        if used > width {
            break;
        }
        clipped.push(c);
    }
    clipped.push('…');
    clipped
}

fn render_keyed(entries: &Map<String, Value>, look: Look) -> String {
    // Union of the rows' keys, in first-seen order.
    let mut columns: Vec<&str> = Vec::new();
    for row in entries.values().filter_map(Value::as_object) {
        for key in row.keys() {
            if !columns.contains(&key.as_str()) {
                columns.push(key);
            }
        }
    }
    let mut builder = Builder::default();
    builder.push_record(std::iter::once("KEY").chain(columns.iter().copied()));
    let mut highlights = Vec::new();
    for (row_i, (key, row)) in entries.iter().enumerate() {
        builder.push_record(
            std::iter::once(key.clone()).chain(
                columns
                    .iter()
                    .map(|column| row.get(*column).map_or_else(String::new, cell)),
            ),
        );
        if look.highlight_key == Some(key.as_str()) {
            // Skip the KEY column; bold every value on the changed row.
            highlights.extend((1..=columns.len()).map(|col| (row_i + 1, col)));
        }
    }
    format!("{}\n", table(builder, look, &highlights))
}

fn render_object(fields: &Map<String, Value>, look: Look) -> String {
    let fields = spread_fields(fields);
    if fields.is_empty() {
        return "(empty)\n".to_owned();
    }
    // Unset fields are counted, not listed: a Jira issue carries hundreds of
    // null custom fields. A record with nothing set shows them all instead.
    let is_set = |value: &Value| summary(value).is_none_or(|text| !text.is_empty());
    let unset = fields.values().filter(|value| !is_set(value)).count();
    let hide_unset = unset < fields.len();
    let has_handle = fields.iter().any(|(key, value)| is_handle(key, value));
    let mut rows: Vec<(Tier, &str, &Value)> = fields
        .iter()
        .filter(|(_, value)| !hide_unset || is_set(value))
        .map(|(key, value)| {
            let tier = tier(key, std::iter::once(value), has_handle);
            (tier, key.as_str(), value)
        })
        .collect();
    rows.sort_by_key(|(tier, ..)| *tier);
    // " FIELD | VALUE ": what is left for the value once the field column
    // and the borders are drawn.
    let field_width = rows
        .iter()
        .map(|(_, key, _)| get_line_width(key))
        .max()
        .unwrap_or(0)
        .max("FIELD".len());
    let value_width = look.width.saturating_sub(field_width + 5).max(20);
    let mut builder = Builder::default();
    builder.push_record(["FIELD", "VALUE"]);
    for (_, key, value) in rows {
        let text = match summary(value) {
            // Text is what a `get` is read for: keep its line breaks and
            // wrap it instead of cutting it to one line.
            Some(text) => {
                let text = text.replace('\r', "").replace('\t', "    ");
                wrap(text.trim_end(), value_width)
            }
            // A nested value with no short form stays one clipped line.
            None => clip(&value.to_string(), value_width),
        };
        builder.push_record([key, text.as_str()]);
    }
    let table = table(builder, look, &[]);
    if !hide_unset || unset == 0 {
        return format!("{table}\n");
    }
    let footer = format!(
        "{} not shown (--format json shows all)",
        count(unset, "empty field")
    );
    format!("{table}\n\n{}\n", dim(&footer, look))
}

/// `text` wrapped to `width` terminal columns, keeping its own line breaks.
/// A line breaks at its last space that fits; a word longer than a line is
/// split.
fn wrap(text: &str, width: usize) -> String {
    // No glyph is wider than two columns, so every line makes progress.
    let width = width.max(2);
    let mut wrapped = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        // `limit`: the byte offset of the first character past `width`.
        while let Some((limit, next)) = overflow(rest, width) {
            let space = if next == ' ' {
                Some(limit)
            } else {
                // Not a space inside the indentation: breaking there would
                // leave a blank line.
                rest[..limit]
                    .rfind(' ')
                    .filter(|space| !rest[..*space].trim().is_empty())
            };
            let (head, tail) = match space {
                Some(space) => (&rest[..space], &rest[space + 1..]),
                None => rest.split_at(limit),
            };
            wrapped.push(head.trim_end());
            rest = tail;
        }
        wrapped.push(rest);
    }
    wrapped.join("\n")
}

/// Where `line` first runs past `width` terminal columns: the byte offset
/// and the character there. `None` when it fits.
fn overflow(line: &str, width: usize) -> Option<(usize, char)> {
    let mut used = 0;
    line.char_indices().find(|(_, c)| {
        used += get_char_width(*c);
        used > width
    })
}

/// The keys that name a nested record, most specific first. The last two
/// are what a value holder carries: `value` for a Jira select option or a
/// Slack topic, `start` for a Notion date.
const LABEL_KEYS: [&str; 12] = [
    "identifier",
    "full_name",
    "login",
    "key",
    "name",
    "displayName",
    "title",
    "email",
    "slug",
    "username",
    "value",
    "start",
];

/// A value's short form: scalars as text (`null` is blank), a nested record
/// as the field that names it (`{"id": ..., "name": "In Progress"}` is
/// `In Progress`), a list as its members' short forms joined. `None` when a
/// nested value has nothing naming it.
fn summary(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some(String::new()),
        Value::String(text) => Some(text.clone()),
        Value::Bool(_) | Value::Number(_) => Some(value.to_string()),
        Value::Array(members) => join_labels(members),
        Value::Object(record) => match record.get("nodes") {
            // A Linear relation is the list it wraps.
            Some(Value::Array(nodes)) if record.len() == 1 => join_labels(nodes),
            _ => label(record).or_else(|| tagged_value(record)),
        },
    }
}

/// Notion tags a value with its `type`, which names the key holding it:
/// `{"type": "number", "number": 42}` is `42`, and a `select` is its
/// option's name.
fn tagged_value(record: &Map<String, Value>) -> Option<String> {
    let kind = record.get("type")?.as_str()?;
    summary(record.get(kind)?)
}

fn label(record: &Map<String, Value>) -> Option<String> {
    LABEL_KEYS.iter().find_map(|key| {
        record
            .get(*key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    })
}

fn join_labels(members: &[Value]) -> Option<String> {
    // Notion rich text is one string, split into runs where its styling
    // changes.
    if !members.is_empty() && members.iter().all(|run| run["plain_text"].is_string()) {
        return Some(
            members
                .iter()
                .filter_map(|run| run["plain_text"].as_str())
                .collect(),
        );
    }
    let labels: Option<Vec<String>> = members
        .iter()
        .map(|member| match member {
            Value::Object(record) => label(record),
            Value::Array(_) | Value::Null => None,
            scalar => summary(scalar),
        })
        .collect();
    Some(labels?.join(", "))
}

/// Newlines, carriage returns, and tabs flattened to spaces.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r', '\t'], " ")
}

/// One table cell: the value's short form on one line, or compact JSON for
/// a nested value with nothing naming it.
fn cell(value: &Value) -> String {
    summary(value).map_or_else(|| value.to_string(), |text| one_line(&text))
}

fn table(builder: Builder, look: Look, highlights: &[(usize, usize)]) -> String {
    let mut table = builder.build();
    // List and object cells are already sized to fit; the truncation here
    // sizes the keyed tables and catches what counting characters misses
    // (wide glyphs).
    table.with(Style::psql()).with(
        Width::truncate(look.width)
            .suffix("…")
            .priority(Priority::max(true)),
    );
    // Color after sizing so cell width does not count ANSI bytes.
    if look.color {
        table.modify(Rows::first(), Color::BOLD);
        for &(row, col) in highlights {
            table.modify((row, col), Color::BOLD);
        }
    }
    // tabled pads every column including the last; keep lines clean.
    table
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The cells of one rendered table line.
    fn cells(line: &str) -> Vec<&str> {
        line.split('|').map(str::trim).collect()
    }

    #[test]
    fn resolves_format_by_precedence() {
        use Format::{Json, Table};
        use FormatArg as Arg;
        // Flag wins over everything.
        assert_eq!(resolve(Arg::Json, Some("table"), false, true), Json);
        assert_eq!(resolve(Arg::Table, Some("json"), true, false), Table);
        // Then FOAC_FORMAT: trimmed, case-insensitive; junk is ignored.
        assert_eq!(resolve(Arg::Auto, Some(" JSON "), false, true), Json);
        assert_eq!(resolve(Arg::Auto, Some("Table"), true, false), Table);
        assert_eq!(resolve(Arg::Auto, Some("yaml"), false, true), Table);
        assert_eq!(resolve(Arg::Auto, Some(""), false, false), Json);
        // Then CI forces JSON even on a TTY.
        assert_eq!(resolve(Arg::Auto, None, true, true), Json);
        // Then the TTY check.
        assert_eq!(resolve(Arg::Auto, None, false, true), Table);
        assert_eq!(resolve(Arg::Auto, None, false, false), Json);
    }

    #[test]
    fn json_mode_matches_display() {
        let value = json!({"issues": {"nodes": [], "pageInfo": {"endCursor": null}}});
        assert_eq!(render(&value, Format::Json, 80), format!("{value}\n"));
    }

    #[test]
    fn depth1_linear_list() {
        let value = json!({"issues": {
            "nodes": [{"identifier": "ENG-1", "title": "Fix login"}],
            "pageInfo": {"hasNextPage": false, "endCursor": "abc"},
        }});
        assert_eq!(
            render(&value, Format::Table, 80),
            " identifier | title\n\
             ------------+-----------\n\
             \x20ENG-1      | Fix login\n\
             \n\
             1 row\n"
        );
    }

    #[test]
    fn depth2_linear_list_ignores_wrapper_metadata() {
        let value = json!({"team": {
            "id": "t1",
            "key": "ENG",
            "cycles": {
                "nodes": [{"number": 4, "name": "Sprint 4"}],
                "pageInfo": {"hasNextPage": true, "endCursor": "xyz"},
            },
        }});
        assert_eq!(
            render(&value, Format::Table, 80),
            " number | name\n\
             --------+----------\n\
             \x204      | Sprint 4\n\
             \n\
             1 row\n\
             More results: endCursor=xyz (see --help for the paging flag)\n"
        );
    }

    #[test]
    fn footer_names_only_the_forward_cursor() {
        // GitHub's four-key pageInfo: only what continues the list is shown,
        // and only while there is a next page.
        let page = |next: Value| {
            json!({
                "items": [{"number": 1}],
                "pageInfo": {
                    "hasNextPage": !next.is_null(),
                    "nextPage": next,
                    "hasPreviousPage": true,
                    "previousPage": 1,
                },
            })
        };
        let more = render(&page(json!(3)), Format::Table, 80);
        assert!(
            more.ends_with("1 row\nMore results: nextPage=3 (see --help for the paging flag)\n")
        );
        let last = render(&page(Value::Null), Format::Table, 80);
        assert!(last.ends_with("\n\n1 row\n"));
        // A heuristic hasNextPage with no cursor beside it.
        let bare = json!({"items": [{"number": 1}], "pageInfo": {"hasNextPage": true}});
        assert!(
            render(&bare, Format::Table, 80)
                .ends_with("1 row\nMore results (see --help for the paging flag)\n")
        );
    }

    #[test]
    fn list_clips_long_text_and_flattens_whitespace() {
        let value = json!({
            "items": [{"body": "line one\nline two\ttabbed", "closed": null, "title": "x".repeat(120)}],
            "pageInfo": {"hasNextPage": false},
        });
        // The all-null column is dropped without being counted as hidden.
        assert_eq!(
            render(&value, Format::Table, 60),
            format!(
                " {:<30} | body\n\
                 {}+{}\n\
                 \x20{}… | line one line two tabbed\n\
                 \n\
                 1 row\n",
                "title",
                "-".repeat(32),
                "-".repeat(26),
                "x".repeat(29),
            )
        );
    }

    #[test]
    fn list_hides_columns_that_do_not_fit() {
        // 80-field REST rows used to become 80 crushed "…" columns; now the
        // leading fields render and the rest are counted.
        let items: Vec<_> = (0..2)
            .map(|row| {
                let mut object = Map::new();
                object.insert("id".into(), json!(row));
                object.insert("name".into(), json!(format!("repo-{row}")));
                for field in 0..80 {
                    object.insert(format!("field_{field:02}"), json!("value"));
                }
                Value::Object(object)
            })
            .collect();
        let value = json!({"items": items, "pageInfo": {"hasNextPage": false}});
        let rendered = render(&value, Format::Table, 40);
        assert!(rendered.starts_with(" name   | field_00 | field_01\n"));
        assert!(rendered.ends_with("2 rows, 79 more columns (--format json shows all)\n"));
        assert!(!rendered.contains("…"));
    }

    #[test]
    fn list_orders_columns_by_what_a_human_reads_first() {
        // GitHub's own order opens with URLs and opaque IDs.
        let value = json!({
            "items": [{
                "url": "https://api.github.com/x/1",
                "id": 5700005242_u64,
                "node_id": "I_kw",
                "number": 155,
                "title": "Add observability",
                "user": {"login": "lra", "id": 1},
                "labels": [{"name": "bug"}, {"name": "docs"}],
                "state": "open",
                "milestone": null,
                "created_at": "2026-10-04T14:03:52Z",
                "reactions": {"total_count": 0},
            }],
            "pageInfo": {"hasNextPage": false},
        });
        let rendered = render(&value, Format::Table, 200);
        let mut lines = rendered.lines();
        assert_eq!(
            cells(lines.next().unwrap()),
            [
                "number",
                "title",
                "state",
                "labels",
                "user",
                "created_at",
                "id",
                "node_id",
                "url"
            ]
        );
        // Nested records show the field that names them.
        assert_eq!(
            cells(lines.nth(1).unwrap()),
            [
                "155",
                "Add observability",
                "open",
                "bug, docs",
                "lra",
                "2026-10-04T14:03:52Z",
                "5700005242",
                "I_kw",
                "https://api.github.com/x/1"
            ]
        );
        // `reactions` has nothing naming it: left to JSON, and counted.
        assert!(rendered.ends_with("1 row, 1 more column (--format json shows all)\n"));
    }

    #[test]
    fn list_shows_an_id_whole_or_not_at_all() {
        let uuid = "b439481e-60b2-4da2-abcf-28707c7dff22";
        let value = json!({
            "items": [{"id": uuid, "identifier": "ENG-1", "title": "t".repeat(50)}],
            "pageInfo": {"hasNextPage": false},
        });
        // Too narrow for the ID: the title takes the spare width instead.
        let narrow = render(&value, Format::Table, 60);
        assert!(!narrow.contains("b439481e"));
        assert!(narrow.contains(&format!("| {}…\n", "t".repeat(43))));
        assert!(narrow.ends_with("1 row, 1 more column (--format json shows all)\n"));
        let wide = render(&value, Format::Table, 120);
        assert_eq!(
            cells(wide.lines().nth(2).unwrap()),
            ["ENG-1", "t".repeat(50).as_str(), uuid]
        );
    }

    #[test]
    fn list_leads_with_the_id_when_it_is_the_handle() {
        // Nothing else names the row (a GitHub comment).
        let comments = json!({
            "items": [{"body": "hello", "id": 12345}],
            "pageInfo": {"hasNextPage": false},
        });
        assert!(render(&comments, Format::Table, 80).starts_with(" id    | body\n"));
        // A short string ID is what commands take (a Slack conversation).
        let conversations = json!({
            "items": [{"is_private": false, "id": "C123", "name": "eng"}],
            "pageInfo": {"hasNextPage": false},
        });
        assert!(
            render(&conversations, Format::Table, 80).starts_with(" id   | name | is_private\n")
        );
    }

    #[test]
    fn list_columns_union_keys_missing_is_empty() {
        let value = json!({
            "items": [
                {"a": "x", "nested": {"skip": 1}},
                {"b": "y"},
            ],
            "pageInfo": {"hasNextPage": false},
        });
        assert_eq!(
            render(&value, Format::Table, 80),
            " a | b\n\
             ---+---\n\
             \x20x |\n\
             \x20  | y\n\
             \n\
             2 rows, 1 more column (--format json shows all)\n"
        );
    }

    #[test]
    fn list_lifts_the_fields_envelope() {
        // Airtable records nest their data under `fields`, in no particular
        // order, under names the user chose: `Name` and `Status` are still
        // the name and the status.
        let value = json!({
            "items": [{
                "id": "rec1",
                "createdTime": "2026-01-01T00:00:00.000Z",
                "fields": {"Notes": "call back", "Status": "Todo", "Name": "Ada"},
            }],
            "pageInfo": {"hasNextPage": false},
        });
        let rendered = render(&value, Format::Table, 80);
        assert_eq!(
            cells(rendered.lines().next().unwrap()),
            ["id", "Name", "Status", "Notes", "createdTime"]
        );
    }

    #[test]
    fn list_puts_jira_fields_in_reading_order() {
        // A Jira issue: a numeric ID beside the key, an `expand` hint, and
        // hundreds of fields in no useful order, custom ones under opaque
        // names.
        let value = json!({
            "items": [{
                "expand": "operations,changelog",
                "id": "10042",
                "self": "https://acme.atlassian.net/rest/api/2/issue/10042",
                "key": "ENG-1",
                "fields": {
                    "customfield_10001": "free text",
                    "assignee": {"accountId": "a1", "displayName": "Ada"},
                    "customfield_10002": {"self": "https://acme.atlassian.net/o/1", "value": "Blue", "id": "1"},
                    "summary": "Fix login",
                    "priority": {"id": "2", "name": "High"},
                    "customfield_10003": null,
                    "status": {"id": "3", "name": "In Progress"},
                },
            }],
            "pageInfo": {"hasNextPage": false},
        });
        let rendered = render(&value, Format::Table, 200);
        let mut lines = rendered.lines();
        assert_eq!(
            cells(lines.next().unwrap()),
            [
                "key",
                "summary",
                "status",
                "priority",
                "assignee",
                "id",
                "self",
                "customfield_10001",
                "customfield_10002",
                "expand"
            ]
        );
        assert_eq!(
            cells(lines.nth(1).unwrap()),
            [
                "ENG-1",
                "Fix login",
                "In Progress",
                "High",
                "Ada",
                "10042",
                "https://acme.atlassian.net/rest/api/2/issue/10042",
                "free text",
                "Blue",
                "operations,changelog"
            ]
        );
        // At 80 columns the plumbing is what gets cut, not the status.
        let narrow = render(&value, Format::Table, 80);
        assert!(narrow.starts_with(" key   | summary   | status      | priority | assignee | id"));
        assert!(!narrow.contains("expand"));
    }

    /// A Notion page as the API returns it: metadata around `properties`,
    /// each property tagged with its type, the title under the user's name
    /// for it.
    fn notion_page() -> Value {
        json!({
            "object": "page",
            "id": "484775bd-c783-82de-b976-0143593501e0",
            "created_time": "2026-07-14T22:23:00.000Z",
            "created_by": {"object": "user", "id": "9a8954d8-33cf-43fc-b11c-000000000000"},
            "parent": {
                "type": "data_source_id",
                "data_source_id": "835775bd-c783-83e7-a4f9-013d3fe728f1",
            },
            "in_trash": false,
            "properties": {
                "Amount": {"id": "a%3Ab", "type": "number", "number": 1200},
                "Client": {"id": "title", "type": "title", "title": [
                    {"type": "text", "plain_text": "Acme ", "annotations": {"bold": false}},
                    {"type": "text", "plain_text": "Corp", "annotations": {"bold": true}},
                ]},
                "Status": {"id": "c", "type": "status", "status": {"id": "1", "name": "Done"}},
                "Tags": {"id": "d", "type": "multi_select", "multi_select": [
                    {"id": "1", "name": "retainer"},
                    {"id": "2", "name": "eu"},
                ]},
                "Due": {"id": "e", "type": "date", "date": {"start": "2026-08-01", "end": null}},
                "Total": {"id": "f", "type": "formula", "formula": {"type": "number", "number": 3}},
                "Site": {"id": "g", "type": "url", "url": null},
            },
            "url": "https://www.notion.so/Acme-Corp-484775bdc78382deb9760143593501e0",
        })
    }

    #[test]
    fn list_reads_notion_properties() {
        let value = json!({"items": [notion_page()], "pageInfo": {"hasNextPage": false}});
        let rendered = render(&value, Format::Table, 300);
        let mut lines = rendered.lines();
        // The title leads under its own name and the page's data comes
        // before Notion's metadata; the UUIDs go with the IDs.
        assert_eq!(
            cells(lines.next().unwrap()),
            [
                "Client",
                "Status",
                "Amount",
                "Tags",
                "Total",
                "object",
                "in_trash",
                "Due",
                "created_time",
                "id",
                "parent",
                "url"
            ]
        );
        assert_eq!(
            cells(lines.nth(1).unwrap()),
            [
                "Acme Corp",
                "Done",
                "1200",
                "retainer, eu",
                "3",
                "page",
                "false",
                "2026-08-01",
                "2026-07-14T22:23:00.000Z",
                "484775bd-c783-82de-b976-0143593501e0",
                "835775bd-c783-83e7-a4f9-013d3fe728f1",
                "https://www.notion.so/Acme-Corp-484775bdc78382deb9760143593501e0"
            ]
        );
        // `created_by` has nothing naming it; the empty `Site` is not counted.
        assert!(rendered.ends_with("1 row, 1 more column (--format json shows all)\n"));
        // At 80 columns the page's own data is what shows.
        let narrow = render(&value, Format::Table, 80);
        assert!(narrow.starts_with(" Client    | Status | Amount | Tags         | Total | object"));
    }

    #[test]
    fn object_reads_notion_properties() {
        let rendered = render(&notion_page(), Format::Table, 100);
        let fields: Vec<&str> = rendered
            .lines()
            .skip(2)
            .take_while(|line| !line.is_empty())
            .map(|line| cells(line)[0])
            .collect();
        assert_eq!(
            fields,
            [
                "Client",
                "Status",
                "Amount",
                "Tags",
                "Total",
                "object",
                "created_by",
                "in_trash",
                "Due",
                "created_time",
                "id",
                "parent",
                "url"
            ]
        );
        assert!(rendered.contains(" Client       | Acme Corp\n"));
        assert!(rendered.ends_with("1 empty field not shown (--format json shows all)\n"));
    }

    #[test]
    fn only_a_notion_pages_properties_are_lifted() {
        // A data source's schema: each entry names itself, so it is a
        // definition, not a page's value.
        let schema = json!({
            "object": "data_source",
            "title": [{"type": "text", "plain_text": "Budget"}],
            "properties": {
                "Amount": {"id": "a", "name": "Amount", "type": "number", "number": {"format": "euro"}},
            },
        });
        let rendered = render(&schema, Format::Table, 80);
        assert!(rendered.contains(" title      | Budget\n"));
        assert!(rendered.contains(" properties | {\"Amount\":{"));
        // Slack's conversation `properties` are not tagged values at all.
        let conversation = json!({
            "items": [{"id": "C123", "name": "eng", "properties": {"tabs": [{"id": "t1", "type": "list"}]}}],
            "pageInfo": {"hasNextPage": false},
        });
        let rendered = render(&conversation, Format::Table, 80);
        assert!(rendered.starts_with(" id   | name\n"));
        assert!(rendered.ends_with("1 row, 1 more column (--format json shows all)\n"));
    }

    #[test]
    fn list_of_bare_values_is_one_column() {
        let value = json!({"items": ["a", "b"], "pageInfo": {"hasNextPage": false}});
        assert_eq!(
            render(&value, Format::Table, 80),
            " value\n\
             -------\n\
             \x20a\n\
             \x20b\n\
             \n\
             2 rows\n"
        );
    }

    #[test]
    fn empty_list_says_so() {
        let value = json!({
            "items": [],
            "pageInfo": {"hasNextPage": false, "nextPage": null, "hasPreviousPage": false, "previousPage": null},
        });
        assert_eq!(render(&value, Format::Table, 80), "No results.\n");
    }

    #[test]
    fn get_rows_flatten_single_object_envelopes() {
        // Slack: scalar envelope beside one object.
        assert_eq!(
            get_row(&json!({ "ok": true, "user": { "id": "U1", "name": "tom" } })),
            json!({ "ok": true, "id": "U1", "name": "tom" })
        );
        // Linear: one-key wrapper.
        assert_eq!(
            get_row(&json!({ "user": { "id": "u-1", "email": "a@b.c" } })),
            json!({ "id": "u-1", "email": "a@b.c" })
        );
        // Jira: wrapper scalars win over inner fields on collision.
        assert_eq!(
            get_row(
                &json!({ "id": "10", "key": "ENG-1", "fields": { "id": "x", "summary": "Fix" } })
            ),
            json!({ "id": "10", "key": "ENG-1", "summary": "Fix" })
        );
        // Flat resources and multi-object bodies pass through unchanged.
        let flat = json!({ "id": 1, "name": "web" });
        assert_eq!(get_row(&flat), flat);
        let multi = json!({ "id": 1, "user": {}, "assignee": {} });
        assert_eq!(get_row(&multi), multi);
        let array = json!({ "id": 1, "labels": [] });
        assert_eq!(get_row(&array), array);
        // A flat resource whose only nested value is incidental is not an
        // envelope: Jira users must not grow avatar-size columns.
        let jira_user = json!({
            "accountId": "a1",
            "displayName": "Ada",
            "avatarUrls": { "16x16": "https://x/16.png", "48x48": "https://x/48.png" },
        });
        assert_eq!(get_row(&jira_user), jira_user);
    }

    #[test]
    fn get_rows_render_as_one_list_table() {
        let rows: Vec<Value> = [
            json!({ "ok": true, "user": { "id": "U1", "name": "tom" } }),
            json!({ "ok": true, "user": { "id": "U2", "name": "mike" } }),
        ]
        .iter()
        .map(get_row)
        .collect();
        assert_eq!(
            render(
                &json!({ "items": rows, "pageInfo": { "count": rows.len() } }),
                Format::Table,
                80
            ),
            " id | name | ok\n\
             ----+------+------\n\
             \x20U1 | tom  | true\n\
             \x20U2 | mike | true\n\
             \n\
             2 rows\n"
        );
    }

    #[test]
    fn keyed_map_renders_one_row_per_key() {
        let value = json!({
            "github": {"enabled": false},
            "linear": {"enabled": true},
        });
        assert_eq!(
            render(&value, Format::Table, 80),
            " KEY    | enabled\n\
             --------+---------\n\
             \x20github | false\n\
             \x20linear | true\n"
        );
    }

    #[test]
    fn keyed_map_keeps_the_rows_own_column_order() {
        // Status first, as the report is built; not alphabetical.
        let value = json!({
            "github": {"status": "authenticated", "account": "lra"},
            "notion": {"status": "unauthenticated", "error": "API token is invalid."},
        });
        assert_eq!(
            render(&value, Format::Table, 80),
            " KEY    | status          | account | error\n\
             --------+-----------------+---------+-----------------------\n\
             \x20github | authenticated   | lra     |\n\
             \x20notion | unauthenticated |         | API token is invalid.\n"
        );
    }

    #[test]
    fn color_bolds_the_header_and_the_highlighted_row_values() {
        let value = json!({
            "github": {"enabled": false},
            "linear": {"enabled": true},
            "sentry": {"enabled": false},
        });
        let look = Look {
            width: 80,
            color: true,
            highlight_key: Some("sentry"),
        };
        assert_eq!(
            render_inner(&value, Format::Table, look),
            " \u{1b}[1mKEY\u{1b}[22m    | \u{1b}[1menabled\u{1b}[22m\n\
             --------+---------\n\
             \x20github | false\n\
             \x20linear | true\n\
             \x20sentry | \u{1b}[1mfalse\u{1b}[22m\n"
        );
    }

    #[test]
    fn color_dims_the_list_footer() {
        let value = json!({
            "items": [{"name": "web"}],
            "pageInfo": {"hasNextPage": true, "endCursor": "c2"},
        });
        let look = Look {
            width: 80,
            color: true,
            highlight_key: None,
        };
        assert!(render_inner(&value, Format::Table, look).ends_with(
            "\n\n\u{1b}[2m1 row\u{1b}[22m\n\
             \u{1b}[2mMore results: endCursor=c2 (see --help for the paging flag)\u{1b}[22m\n"
        ));
    }

    #[test]
    fn without_color_nothing_is_styled() {
        let value = json!({
            "github": {"enabled": true},
            "sentry": {"enabled": false},
        });
        let look = Look {
            width: 80,
            color: false,
            highlight_key: Some("sentry"),
        };
        assert!(!render_inner(&value, Format::Table, look).contains('\u{1b}'));
        // JSON never is, whatever the terminal.
        let look = Look {
            color: true,
            ..look
        };
        assert_eq!(
            render_inner(&value, Format::Json, look),
            format!("{value}\n")
        );
    }

    #[test]
    fn object_renders_field_value() {
        let value = json!({"identifier": "ENG-1", "title": "Fix login"});
        assert_eq!(
            render(&value, Format::Table, 80),
            " FIELD      | VALUE\n\
             ------------+-----------\n\
             \x20identifier | ENG-1\n\
             \x20title      | Fix login\n"
        );
    }

    #[test]
    fn object_shows_nested_records_by_name() {
        // IssueGet: labels.nodes has no pageInfo sibling, so this is not a
        // list; the relation renders as its members' names.
        let value = json!({"issue": {
            "identifier": "ENG-1",
            "assignee": null,
            "state": {"id": "s1", "name": "In Progress", "type": "started"},
            "team": {"id": "t1", "key": "ENG"},
            "labels": {"nodes": [{"id": "l1", "name": "bug"}, {"id": "l2", "name": "p1"}]},
            "children": {"nodes": []},
        }});
        assert_eq!(
            render(&value, Format::Table, 80),
            " FIELD      | VALUE\n\
             ------------+-------------\n\
             \x20identifier | ENG-1\n\
             \x20state      | In Progress\n\
             \x20labels     | bug, p1\n\
             \x20team       | ENG\n\
             \n\
             2 empty fields not shown (--format json shows all)\n"
        );
    }

    #[test]
    fn object_with_nothing_set_lists_every_field() {
        let value = json!({"assignee": null, "labels": []});
        assert_eq!(
            render(&value, Format::Table, 80),
            " FIELD    | VALUE\n\
             ----------+-------\n\
             \x20assignee |\n\
             \x20labels   |\n"
        );
    }

    #[test]
    fn mutation_wrapper_unwraps_to_field_value() {
        let value = json!({"issueUpdate": {"success": true, "issue": {"id": "i1"}}});
        assert_eq!(
            render(&value, Format::Table, 80),
            " FIELD   | VALUE\n\
             ---------+-------------\n\
             \x20success | true\n\
             \x20issue   | {\"id\":\"i1\"}\n"
        );
        let value = json!({"issueCreate": {
            "success": true,
            "issue": {"id": "i1", "identifier": "ENG-7", "title": "Fix login"},
        }});
        assert!(render(&value, Format::Table, 80).ends_with(" issue   | ENG-7\n"));
    }

    #[test]
    fn object_wraps_long_text_and_keeps_line_breaks() {
        let value = json!({
            "description": "first line\r\n\r\nsecond paragraph that is long enough to wrap around the value column\n",
            "title": "T",
        });
        assert_eq!(
            render(&value, Format::Table, 40),
            " FIELD       | VALUE\n\
             -------------+--------------------------\n\
             \x20title       | T\n\
             \x20description | first line\n\
             \x20            |\n\
             \x20            | second paragraph that is\n\
             \x20            | long enough to wrap\n\
             \x20            | around the value column\n"
        );
    }

    #[test]
    fn object_orders_rows_and_clips_unnamed_nested_values() {
        let value = json!({
            "url": "https://api.example.com/issues/1",
            "id": 7,
            "number": 1,
            "created_at": "2026-01-01T00:00:00Z",
            "state": "open",
            "closed_at": null,
            "reactions": {"total_count": 0, "url": format!("https://api.example.com/{}", "a".repeat(60))},
        });
        let rendered = render(&value, Format::Table, 60);
        let fields: Vec<&str> = rendered
            .lines()
            .skip(2)
            .take_while(|line| !line.is_empty())
            .map(|line| cells(line)[0])
            .collect();
        assert_eq!(
            fields,
            ["number", "state", "reactions", "created_at", "id", "url"]
        );
        assert!(rendered.ends_with("\n\n1 empty field not shown (--format json shows all)\n"));
        let reactions = rendered.lines().nth(4).unwrap();
        assert!(reactions.starts_with(" reactions  | {\"total_count\":0,"));
        assert!(reactions.ends_with('…'));
        assert!(rendered.lines().all(|line| line.chars().count() <= 60));
    }

    #[test]
    fn wrap_breaks_at_spaces_and_splits_long_words() {
        assert_eq!(wrap("one two three", 7), "one two\nthree");
        assert_eq!(wrap("one two three", 8), "one two\nthree");
        assert_eq!(wrap("abcdefghij", 4), "abcd\nefgh\nij");
        // Indentation is not a break point.
        assert_eq!(wrap("  abcdefghij", 6), "  abcd\nefghij");
        assert_eq!(wrap("  - one two", 8), "  - one\ntwo");
        assert_eq!(wrap("a\n\nb", 10), "a\n\nb");
        assert_eq!(wrap("", 10), "");
        // Counted in terminal columns, not bytes: an accented letter takes
        // one, an emoji or a CJK glyph two.
        assert_eq!(wrap("ééé ééé", 3), "ééé\nééé");
        assert_eq!(wrap("日本語日本語", 6), "日本語\n日本語");
        assert_eq!(wrap("日本", 1), "日\n本");
    }

    #[test]
    fn wide_glyphs_never_push_a_table_past_the_terminal() {
        let title = "🚀 ship 日本語 ".repeat(8);
        let list = json!({
            "items": [{"identifier": "ENG-1", "title": title, "state": "Todo"}],
            "pageInfo": {"hasNextPage": false},
        });
        let object = json!({"identifier": "ENG-1", "description": title});
        // The list clips its title once; the object wraps, cutting nothing.
        // Any other ellipsis would be tabled's own truncation stepping in
        // because foac measured a line too short.
        for (value, clipped) in [(list, 1), (object, 0)] {
            let rendered = render(&value, Format::Table, 60);
            let table = rendered.split("\n\n").next().unwrap();
            assert!(table.contains("日本語"));
            assert!(table.lines().all(|line| get_line_width(line) <= 60));
            assert_eq!(table.matches('…').count(), clipped);
        }
    }

    #[test]
    fn empty_object_prints_empty_marker() {
        assert_eq!(render(&json!({}), Format::Table, 80), "(empty)\n");
    }

    #[test]
    fn error_headline_reads_each_provider_shape() {
        for (body, headline) in [
            // GitHub, Notion, Neon, Axiom
            (
                json!({"message": "Not Found", "status": "404"}),
                "Not Found",
            ),
            // Slack, Firecrawl
            (
                json!({"ok": false, "error": "channel_not_found"}),
                "channel_not_found",
            ),
            // Vercel, Airtable
            (
                json!({"error": {"code": "not_found", "message": "Project not found."}}),
                "Project not found.",
            ),
            // Linear, Fireflies
            (
                json!({"errors": [{"message": "Entity not found: Issue", "path": ["issue"]}]}),
                "Entity not found: Issue",
            ),
            // Confluence v2
            (
                json!({"errors": [{"status": 404, "code": "NOT_FOUND", "title": "Not Found", "detail": null}]}),
                "Not Found",
            ),
            // Jira
            (
                json!({"errorMessages": ["Issue does not exist."], "errors": {}}),
                "Issue does not exist.",
            ),
            (
                json!({"errorMessages": [], "errors": {"summary": "You must specify a summary"}}),
                "summary: You must specify a summary",
            ),
            // Sentry
            (
                json!({"detail": "The requested resource does not exist"}),
                "The requested resource does not exist",
            ),
        ] {
            assert_eq!(error_headline(&body.to_string()).as_deref(), Some(headline));
        }
        // foac's own errors are plain text; a body with no message has no headline.
        assert_eq!(error_headline("jira is disabled by /x/.foac.toml"), None);
        assert_eq!(error_headline(r#"{"status": 500}"#), None);
        assert_eq!(error_headline(r#"["message"]"#), None);
    }

    #[test]
    fn human_errors_get_a_headline_and_keep_the_body() {
        let body = r#"{"message":"Not Found","status":"404"}"#;
        assert_eq!(
            render_error(body, false),
            format!("error: Not Found\n{body}\n")
        );
        assert_eq!(
            render_error("jira is disabled", false),
            "error: jira is disabled\n"
        );
        assert_eq!(
            render_error(body, true),
            format!("\u{1b}[1;31merror:\u{1b}[0m Not Found\n\u{1b}[2m{body}\u{1b}[0m\n")
        );
    }
}
