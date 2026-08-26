//! Pipe mode for `get` verbs: with `list` output piped on stdin and the
//! identifying positional argument omitted, `get` joins on one field of each
//! list item and runs one get per extracted value. Single-value `get`
//! behavior is unchanged.

use std::io::{IsTerminal, Read};

use serde_json::Value;

/// The `--from` flag carried by every `get` verb with an identifying
/// positional argument. `pub` because provider command enums are `pub`.
#[derive(clap::Args, Default)]
pub struct FromFlag {
    /// With `list` JSON piped on stdin and the positional argument omitted,
    /// run one get per list item, joining on this field; dots reach into
    /// nested objects (`profile.email`)
    #[arg(long)]
    pub from: Option<String>,
}

/// A get that reached the API and came back "no such resource"; displays as
/// the API's error body. Pipe mode counts these as misses; any other error
/// (auth, rate limit, network) is a failure and prints as itself.
#[derive(Debug)]
pub(crate) struct NotFound(pub(crate) String);

impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotFound {}

/// Run a `get` verb through `get_one`, which fetches one value's response
/// body. With the positional present, behavior is unchanged: one get, its
/// error propagated. Without it, stdin must be piped `list` output; each
/// extracted value is fetched, successes stream as JSON documents on stdout,
/// misses ([`NotFound`]) end in a single stderr summary line, and any other
/// failure prints its error as it happens. The command succeeds when every
/// value was fetched, or when at least one was and the rest were misses.
pub(crate) fn run_get<V>(
    value: Option<V>,
    from: FromFlag,
    format: crate::output::Format,
    mut get_one: impl FnMut(V) -> Result<Value, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    V: Clone + std::fmt::Display + std::str::FromStr,
{
    if let Some(value) = value {
        if from.from.is_some() {
            return Err(
                "--from reads values from piped stdin; omit the positional argument".into(),
            );
        }
        let body = get_one(value)?;
        crate::output::print(&body, format);
        return Ok(());
    }
    if std::io::stdin().is_terminal() {
        return Err("missing a value to get: pass it as an argument, or pipe `list` output and join with --from <field>".into());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let (values, skipped) = extract_values(&input, from.from.as_deref())?;
    if skipped > 0 {
        let field = from.from.as_deref().unwrap_or_default();
        eprintln!(
            "skipped {skipped} of {} piped items with no {field}",
            values.len() + skipped
        );
    }
    run_values(parse_values(values)?, format, get_one)
}

/// Piped values re-enter the type the positional argument would have had
/// from clap (GitHub's numeric IDs), so a wrong --from field is a hard
/// error, not a run of 404 "misses".
fn parse_values<V: std::str::FromStr>(
    values: Vec<String>,
) -> Result<Vec<V>, Box<dyn std::error::Error>> {
    values
        .into_iter()
        .map(|raw| {
            raw.parse().map_err(|_| {
                format!("piped value {raw:?} is not valid for this get's identifying argument; check the --from field").into()
            })
        })
        .collect()
}

fn run_values<V: Clone + std::fmt::Display>(
    values: Vec<V>,
    format: crate::output::Format,
    mut get_one: impl FnMut(V) -> Result<Value, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let total = values.len();
    let mut missed = Vec::new();
    let mut failed = 0;
    // JSON streams one document per get; a table renders once at the end,
    // one row per response, instead of one boxed table per response.
    let mut rows = Vec::new();
    for value in values {
        match get_one(value.clone()) {
            Ok(body) => match format {
                crate::output::Format::Json => crate::output::print(&body, format),
                crate::output::Format::Table => rows.push(body),
            },
            Err(error) if error.is::<NotFound>() => missed.push(value.to_string()),
            Err(error) => {
                failed += 1;
                eprintln!("{value}: {error}");
            }
        }
    }
    if !rows.is_empty() {
        crate::output::print_get_rows(&rows);
    }
    if !missed.is_empty() {
        let summary = format!(
            "{} of {total} not found: {}",
            missed.len(),
            missed.join(", ")
        );
        if missed.len() == total {
            return Err(summary.into());
        }
        eprintln!("{summary}");
    }
    if failed > 0 {
        return Err(format!("{failed} of {total} gets failed").into());
    }
    Ok(())
}

/// The values to get, plus how many list items were skipped for lacking the
/// join field: for a piped JSON list, `field` of each item (REST `items`, or
/// the first `nodes` array in Linear's GraphQL nesting); for anything else,
/// one value per non-empty line.
fn extract_values(
    input: &str,
    field: Option<&str>,
) -> Result<(Vec<String>, usize), Box<dyn std::error::Error>> {
    let document = serde_json::from_str::<Value>(input)
        .ok()
        .filter(|value| value.is_object() || value.is_array());
    let Some(document) = document else {
        if let Some(field) = field {
            return Err(format!("--from {field} requires `list` JSON piped on stdin").into());
        }
        let values: Vec<String> = input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();
        if values.is_empty() {
            return Err(
                "nothing piped on stdin; pass a value as an argument or pipe `list` output".into(),
            );
        }
        return Ok((values, 0));
    };
    let field = field.ok_or("piped JSON needs --from <field> to name the join key")?;
    let items = list_items(&document).ok_or("piped JSON has no `items` or `nodes` array")?;
    // Dots descend into nested objects (Slack's `profile.email`).
    // ponytail: a field name containing a literal dot cannot be addressed.
    let values: Vec<String> = items
        .iter()
        .filter_map(|item| {
            crate::rest::id_string(field.split('.').fold(item, |value, key| &value[key]))
        })
        .collect();
    if values.is_empty() && !items.is_empty() {
        return Err(format!("no {field} values in the piped list items").into());
    }
    let skipped = items.len() - values.len();
    Ok((values, skipped))
}

fn list_items(document: &Value) -> Option<&Vec<Value>> {
    if let Some(items) = document.as_array() {
        return Some(items);
    }
    if let Some(items) = document.get("items").and_then(Value::as_array) {
        return Some(items);
    }
    find_nodes(document)
}

/// Depth-first search for the first `nodes` array, covering Linear's
/// depth-1 (`{"users":{"nodes":[...]}}`) and depth-2 (`{"team":{"cycles":
/// {"nodes":[...]}}}`) list shapes.
fn find_nodes(value: &Value) -> Option<&Vec<Value>> {
    let object = value.as_object()?;
    // `nodes` only counts as the list when `pageInfo` sits beside it, the
    // same rule the table renderer applies: bare `nodes` (issue labels,
    // team members) are relation fields, not the piped list.
    if let (Some(Value::Array(nodes)), Some(_)) = (object.get("nodes"), object.get("pageInfo")) {
        return Some(nodes);
    }
    object.values().find_map(find_nodes)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn extracts_field_values_from_rest_items() {
        let input =
            r#"{"items":[{"name":"web","id":1},{"name":"api","id":2},{"id":3}],"pageInfo":{}}"#;
        // The item with no name is skipped and counted.
        assert_eq!(
            extract_values(input, Some("name")).unwrap(),
            (vec!["web".into(), "api".into()], 1)
        );
        // Numeric join keys (GitHub numbers, IDs) become strings.
        assert_eq!(
            extract_values(input, Some("id")).unwrap(),
            (vec!["1".into(), "2".into(), "3".into()], 0)
        );
    }

    #[test]
    fn extracts_field_values_from_linear_nodes() {
        let depth1 = r#"{"users":{"nodes":[{"email":"a@b.c"},{"email":"d@e.f"}],"pageInfo":{}}}"#;
        assert_eq!(
            extract_values(depth1, Some("email")).unwrap().0,
            vec!["a@b.c", "d@e.f"]
        );
        let depth2 = r#"{"team":{"id":"t1","cycles":{"nodes":[{"number":4}],"pageInfo":{}}}}"#;
        assert_eq!(extract_values(depth2, Some("number")).unwrap().0, vec!["4"]);
        let array = r#"[{"name":"web"}]"#;
        assert_eq!(extract_values(array, Some("name")).unwrap().0, vec!["web"]);
    }

    #[test]
    fn dotted_from_reaches_nested_fields() {
        // Slack members carry email at profile.email.
        let input = r#"{"items":[{"id":"U1","profile":{"email":"a@b.c"}},{"id":"U2","profile":{}}],"pageInfo":{}}"#;
        assert_eq!(
            extract_values(input, Some("profile.email")).unwrap(),
            (vec!["a@b.c".into()], 1)
        );
    }

    #[test]
    fn relation_nodes_without_page_info_are_not_the_list() {
        // A relation (`labels.nodes`, no pageInfo) before the real connection
        // must not win the join, whatever the field order.
        let relation_first = r#"{"issue":{"labels":{"nodes":[{"name":"bug"}]},"children":{"nodes":[{"identifier":"ENG-2"}],"pageInfo":{}}}}"#;
        assert_eq!(
            extract_values(relation_first, Some("identifier"))
                .unwrap()
                .0,
            vec!["ENG-2"]
        );
        // Only bare relation nodes: not a list shape at all.
        let error = extract_values(
            r#"{"issue":{"labels":{"nodes":[{"name":"bug"}]}}}"#,
            Some("name"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("no `items` or `nodes` array"));
    }

    #[test]
    fn non_json_input_is_one_value_per_line() {
        assert_eq!(
            extract_values("a@b.c\n\n  d@e.f \n", None).unwrap().0,
            vec!["a@b.c", "d@e.f"]
        );
        // A single numeric line parses as JSON but is still line input.
        assert_eq!(extract_values("123\n", None).unwrap().0, vec!["123"]);
        assert!(extract_values("", None).is_err());
        assert!(extract_values("a@b.c\n", Some("email")).is_err());
    }

    #[test]
    fn json_input_requires_from_and_a_known_list_shape() {
        let items = r#"{"items":[{"name":"web"}],"pageInfo":{}}"#;
        assert!(extract_values(items, None).is_err());
        assert!(extract_values(r#"{"foo":1}"#, Some("name")).is_err());
        assert!(extract_values(items, Some("nope")).is_err());
        // An empty list is a legitimate empty join, not an error.
        assert_eq!(
            extract_values(r#"{"items":[],"pageInfo":{}}"#, Some("name")).unwrap(),
            (Vec::new(), 0)
        );
    }

    #[test]
    fn run_values_separates_misses_from_failures() {
        let format = crate::output::Format::Json;
        let get = |value: String| -> Result<Value, Box<dyn std::error::Error>> {
            if value.starts_with("ok") {
                Ok(json!({ "value": value }))
            } else if value.starts_with("miss") {
                Err(NotFound(format!("no such {value}")).into())
            } else {
                Err("429 rate limited".into())
            }
        };
        let values = |list: &[&str]| {
            list.iter()
                .map(|v| (*v).to_owned())
                .collect::<Vec<String>>()
        };
        // Misses alongside a success are a summarized, successful run.
        assert!(run_values(values(&["ok-1", "ok-2"]), format, get).is_ok());
        assert!(run_values(values(&["ok-1", "miss"]), format, get).is_ok());
        assert!(run_values(values(&[]), format, get).is_ok());
        let error = run_values(values(&["miss-1", "miss-2"]), format, get).unwrap_err();
        assert_eq!(error.to_string(), "2 of 2 not found: miss-1, miss-2");
        // Any real failure fails the command, even beside successes.
        let error = run_values(values(&["ok-1", "fail"]), format, get).unwrap_err();
        assert_eq!(error.to_string(), "1 of 2 gets failed");
        let error = run_values(values(&["fail-1", "fail-2"]), format, get).unwrap_err();
        assert_eq!(error.to_string(), "2 of 2 gets failed");
    }

    #[test]
    fn piped_values_must_parse_as_the_argument_type() {
        assert_eq!(
            parse_values::<u64>(vec!["1".into(), "42".into()]).unwrap(),
            vec![1, 42]
        );
        let error = parse_values::<u64>(vec!["1".into(), "Fix the build".into()]).unwrap_err();
        assert!(error.to_string().contains("\"Fix the build\""));
        // String arguments accept anything, as before.
        assert_eq!(
            parse_values::<String>(vec!["ENG-1".into()]).unwrap(),
            vec!["ENG-1"]
        );
    }

    #[test]
    fn run_get_rejects_from_alongside_a_positional() {
        let error = run_get(
            Some("value".to_owned()),
            FromFlag {
                from: Some("email".into()),
            },
            crate::output::Format::Json,
            |_| Ok(json!({})),
        )
        .unwrap_err();
        assert!(error.to_string().contains("--from"));
    }
}
