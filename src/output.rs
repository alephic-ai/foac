//! Shared success-output printer for the Linear and GitHub providers: compact
//! JSON for machines, tables sized to the terminal for humans.

use std::io::IsTerminal;

use serde_json::{Map, Value};
use tabled::builder::Builder;
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

/// JSON uses [`print`]; table mode writes `text` as-is (auth summaries).
pub fn print_text(text: &str, value: &Value, format: Format) {
    match format {
        Format::Json => print(value, format),
        Format::Table => print!("{text}"),
    }
}

fn emit(value: &Value, format: Format, highlight_key: Option<&str>) {
    let width = terminal_size::terminal_size().map_or(80, |(w, _)| w.0 as usize);
    let highlight_key = highlight_key.filter(|_| format == Format::Table && color_enabled());
    print!("{}", render_inner(value, format, width, highlight_key));
}

fn color_enabled() -> bool {
    std::io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
}

#[cfg(test)]
fn render(value: &Value, format: Format, width: usize) -> String {
    render_inner(value, format, width, None)
}

fn render_inner(
    value: &Value,
    format: Format,
    width: usize,
    highlight_key: Option<&str>,
) -> String {
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
        return render_list(items, page, width);
    }
    if root.len() == 1 {
        let inner = root.values().next().unwrap();
        if let Some(inner) = inner.as_object() {
            // Depth-1 Linear list: the wrapper value is the connection.
            if let Some((nodes, page)) = connection(inner) {
                return render_list(nodes, page, width);
            }
            // Depth-2 Linear list (CommentList, CycleList): a field of the
            // wrapper is the connection; sibling fields are metadata.
            if let Some((nodes, page)) = inner
                .values()
                .find_map(|f| f.as_object().and_then(connection))
            {
                return render_list(nodes, page, width);
            }
            // Single object behind a one-key wrapper (gets, mutations).
            return render_object(inner, width);
        }
    }
    // Keyed map (provider list, auth status): one row per key.
    if root.len() > 1 && root.values().all(Value::is_object) {
        return render_keyed(root, width, highlight_key);
    }
    render_object(root, width)
}

/// `nodes` only counts as a list when `pageInfo` sits next to it; bare
/// `nodes` (issue labels, team members) are relation fields.
fn connection(obj: &Map<String, Value>) -> Option<(&Vec<Value>, &Map<String, Value>)> {
    match (obj.get("nodes"), obj.get("pageInfo")) {
        (Some(Value::Array(nodes)), Some(Value::Object(page))) => Some((nodes, page)),
        _ => None,
    }
}

fn render_list(rows: &[Value], page: &Map<String, Value>, width: usize) -> String {
    let footer = page
        .iter()
        .map(|(key, value)| format!("{key}={}", cell(value)))
        .collect::<Vec<_>>()
        .join(" ");
    if rows.is_empty() {
        return format!("{footer}\n");
    }
    // Scalar keys only, union across rows; BTreeSet keeps them sorted.
    let columns: std::collections::BTreeSet<&str> = rows
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|row| {
            row.iter()
                .filter(|(_, value)| !value.is_object() && !value.is_array())
                .map(|(key, _)| key.as_str())
        })
        .collect();
    let mut builder = Builder::default();
    builder.push_record(columns.iter().copied());
    for row in rows {
        builder.push_record(
            columns
                .iter()
                .map(|key| row.get(*key).map_or_else(String::new, cell)),
        );
    }
    format!("{}\n\n{footer}\n", table(builder, width, &[]))
}

fn render_keyed(entries: &Map<String, Value>, width: usize, highlight_key: Option<&str>) -> String {
    let columns: std::collections::BTreeSet<&str> = entries
        .values()
        .filter_map(Value::as_object)
        .flat_map(|row| row.keys().map(String::as_str))
        .collect();
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
        if highlight_key == Some(key.as_str()) {
            // Skip the KEY column; bold every value on the changed row.
            highlights.extend((1..=columns.len()).map(|col| (row_i + 1, col)));
        }
    }
    format!("{}\n", table(builder, width, &highlights))
}

fn render_object(fields: &Map<String, Value>, width: usize) -> String {
    if fields.is_empty() {
        return "(empty)\n".to_owned();
    }
    let mut builder = Builder::default();
    builder.push_record(["FIELD", "VALUE"]);
    for (key, value) in fields {
        builder.push_record([key.clone(), cell(value)]);
    }
    format!("{}\n", table(builder, width, &[]))
}

/// Strings render raw (no JSON quoting) with newlines/CR/tabs flattened to
/// spaces; everything else (null, numbers, nested JSON) renders compact.
fn cell(value: &Value) -> String {
    match value {
        Value::String(text) => text.replace(['\n', '\r', '\t'], " "),
        other => other.to_string(),
    }
}

fn table(builder: Builder, width: usize, highlights: &[(usize, usize)]) -> String {
    let mut table = builder.build();
    table.with(Style::psql()).with(
        Width::truncate(width)
            .suffix("…")
            .priority(Priority::max(true)),
    );
    // Color after truncate so cell width does not count ANSI bytes.
    for &(row, col) in highlights {
        table.modify((row, col), Color::BOLD);
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
             endCursor=abc hasNextPage=false\n"
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
            " name     | number\n\
             ----------+--------\n\
             \x20Sprint 4 | 4\n\
             \n\
             endCursor=xyz hasNextPage=true\n"
        );
    }

    #[test]
    fn github_list_flattens_and_truncates_to_width() {
        let value = json!({
            "items": [{"body": "line one\nline two\ttabbed", "closed": null, "title": "x".repeat(120)}],
            "pageInfo": {"hasNextPage": false, "nextPage": null, "hasPreviousPage": false, "previousPage": null},
        });
        assert_eq!(
            render(&value, Format::Table, 60),
            " body                    | closed | title\n\
             -------------------------+--------+-------------------------\n\
             \x20line one line two tabb… | null   | xxxxxxxxxxxxxxxxxxxxxx…\n\
             \n\
             hasNextPage=false hasPreviousPage=false nextPage=null previousPage=null\n"
        );
    }

    #[test]
    fn list_columns_union_scalar_keys_missing_is_empty() {
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
             hasNextPage=false\n"
        );
    }

    #[test]
    fn empty_list_prints_footer_only() {
        let value = json!({
            "items": [],
            "pageInfo": {"hasNextPage": false, "nextPage": null, "hasPreviousPage": false, "previousPage": null},
        });
        assert_eq!(
            render(&value, Format::Table, 80),
            "hasNextPage=false hasPreviousPage=false nextPage=null previousPage=null\n"
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
    fn keyed_map_highlights_the_named_row_values() {
        let value = json!({
            "github": {"enabled": false},
            "linear": {"enabled": true},
            "sentry": {"enabled": false},
        });
        assert_eq!(
            render_inner(&value, Format::Table, 80, Some("sentry")),
            " KEY    | enabled\n\
             --------+---------\n\
             \x20github | false\n\
             \x20linear | true\n\
             \x20sentry | \u{1b}[1mfalse\u{1b}[22m\n"
        );
    }

    #[test]
    fn json_mode_ignores_highlight() {
        let value = json!({
            "github": {"enabled": true},
            "sentry": {"enabled": false},
        });
        assert_eq!(
            render_inner(&value, Format::Json, 80, Some("sentry")),
            format!("{value}\n")
        );
    }

    #[test]
    fn github_object_renders_field_value() {
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
    fn relation_nodes_without_page_info_stay_a_single_object() {
        // IssueGet: labels.nodes has no pageInfo sibling, so this is not a list.
        let value = json!({"issue": {
            "identifier": "ENG-1",
            "labels": {"nodes": [{"name": "bug"}]},
        }});
        assert_eq!(
            render(&value, Format::Table, 80),
            " FIELD      | VALUE\n\
             ------------+----------------------------\n\
             \x20identifier | ENG-1\n\
             \x20labels     | {\"nodes\":[{\"name\":\"bug\"}]}\n"
        );
    }

    #[test]
    fn mutation_wrapper_unwraps_to_field_value() {
        let value = json!({"issueUpdate": {"success": true, "issue": {"id": "i1"}}});
        assert_eq!(
            render(&value, Format::Table, 80),
            " FIELD   | VALUE\n\
             ---------+-------------\n\
             \x20issue   | {\"id\":\"i1\"}\n\
             \x20success | true\n"
        );
    }

    #[test]
    fn empty_object_prints_empty_marker() {
        assert_eq!(render(&json!({}), Format::Table, 80), "(empty)\n");
    }
}
