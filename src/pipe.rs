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
    /// run one get per list item, joining on this field
    #[arg(long)]
    pub from: Option<String>,
}

/// Run a `get` verb through `get_one`, which fetches one value's response
/// body. With the positional present, behavior is unchanged: one get, its
/// error propagated. Without it, stdin must be piped `list` output; each
/// extracted value is fetched, successes stream as JSON documents on stdout,
/// misses end in a single stderr summary line, and the command fails only
/// when every get failed.
pub(crate) fn run_get(
    value: Option<String>,
    from: FromFlag,
    format: crate::output::Format,
    mut get_one: impl FnMut(String) -> Result<Value, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
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
    run_values(
        extract_values(&input, from.from.as_deref())?,
        format,
        get_one,
    )
}

fn run_values(
    values: Vec<String>,
    format: crate::output::Format,
    mut get_one: impl FnMut(String) -> Result<Value, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let total = values.len();
    let mut missed = Vec::new();
    // JSON streams one document per get; a table renders once at the end,
    // one row per response, instead of one boxed table per response.
    let mut rows = Vec::new();
    for value in values {
        match get_one(value.clone()) {
            Ok(body) => match format {
                crate::output::Format::Json => crate::output::print(&body, format),
                crate::output::Format::Table => rows.push(body),
            },
            Err(_) => missed.push(value),
        }
    }
    if !rows.is_empty() {
        crate::output::print_get_rows(&rows);
    }
    if missed.is_empty() {
        return Ok(());
    }
    let summary = format!(
        "{} of {total} not found: {}",
        missed.len(),
        missed.join(", ")
    );
    if missed.len() == total {
        return Err(summary.into());
    }
    eprintln!("{summary}");
    Ok(())
}

/// The values to get: for a piped JSON list, `field` of each item (REST
/// `items`, or the first `nodes` array in Linear's GraphQL nesting); for
/// anything else, one value per non-empty line.
fn extract_values(
    input: &str,
    field: Option<&str>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
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
        return Ok(values);
    };
    let field = field.ok_or("piped JSON needs --from <field> to name the join key")?;
    let items = list_items(&document).ok_or("piped JSON has no `items` or `nodes` array")?;
    let values: Vec<String> = items
        .iter()
        .filter_map(|item| crate::rest::id_string(&item[field]))
        .collect();
    if values.is_empty() && !items.is_empty() {
        return Err(format!("no {field} values in the piped list items").into());
    }
    Ok(values)
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
    if let Some(nodes) = object.get("nodes").and_then(Value::as_array) {
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
        assert_eq!(
            extract_values(input, Some("name")).unwrap(),
            vec!["web", "api"]
        );
        // Numeric join keys (GitHub numbers, IDs) become strings.
        assert_eq!(
            extract_values(input, Some("id")).unwrap(),
            vec!["1", "2", "3"]
        );
    }

    #[test]
    fn extracts_field_values_from_linear_nodes() {
        let depth1 = r#"{"users":{"nodes":[{"email":"a@b.c"},{"email":"d@e.f"}],"pageInfo":{}}}"#;
        assert_eq!(
            extract_values(depth1, Some("email")).unwrap(),
            vec!["a@b.c", "d@e.f"]
        );
        let depth2 = r#"{"team":{"id":"t1","cycles":{"nodes":[{"number":4}],"pageInfo":{}}}}"#;
        assert_eq!(extract_values(depth2, Some("number")).unwrap(), vec!["4"]);
        let array = r#"[{"name":"web"}]"#;
        assert_eq!(extract_values(array, Some("name")).unwrap(), vec!["web"]);
    }

    #[test]
    fn non_json_input_is_one_value_per_line() {
        assert_eq!(
            extract_values("a@b.c\n\n  d@e.f \n", None).unwrap(),
            vec!["a@b.c", "d@e.f"]
        );
        // A single numeric line parses as JSON but is still line input.
        assert_eq!(extract_values("123\n", None).unwrap(), vec!["123"]);
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
            Vec::<String>::new()
        );
    }

    #[test]
    fn run_values_fails_only_when_every_get_failed() {
        let format = crate::output::Format::Json;
        let get = |value: String| -> Result<Value, Box<dyn std::error::Error>> {
            if value.starts_with("ok") {
                Ok(json!({ "value": value }))
            } else {
                Err("not found".into())
            }
        };
        assert!(run_values(vec!["ok-1".into(), "ok-2".into()], format, get).is_ok());
        assert!(run_values(vec!["ok-1".into(), "miss".into()], format, get).is_ok());
        assert!(run_values(Vec::new(), format, get).is_ok());
        let error = run_values(vec!["miss-1".into(), "miss-2".into()], format, get).unwrap_err();
        assert_eq!(error.to_string(), "2 of 2 not found: miss-1, miss-2");
    }

    #[test]
    fn run_get_rejects_from_alongside_a_positional() {
        let error = run_get(
            Some("value".into()),
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
