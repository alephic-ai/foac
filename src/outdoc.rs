//! Rendered "Output:" sections for provider-command `--help`.
//!
//! Every JSON-emitting provider verb attaches one of these strings via
//! `#[command(after_long_help = ...)]`; a test in `main.rs` walks the clap
//! tree and fails when a leaf command lacks its contract. Wording lives here
//! so help stays uniform across providers. These sections document existing
//! behavior only; they never change what a command prints.

/// The section header, in the same style clap gives "Usage:" and "Options:".
/// anstream strips the styling wherever clap itself prints unstyled (pipes,
/// NO_COLOR), so agents always read a plain `Output:`.
fn header() -> String {
    let styles = clap::builder::Styles::default();
    let style = styles.get_header();
    format!("{}Output:{}", style.render(), style.render_reset())
}

fn section(lines: &[&str]) -> String {
    let mut out = header();
    for line in lines {
        out.push_str("\n  ");
        out.push_str(line);
    }
    out
}

/// Escape hatch for envelopes no family helper covers (for example the
/// two-key Linear workspace get). Callers still get the uniform header.
pub(crate) fn lines(lines: &[&str]) -> String {
    section(lines)
}

fn join_clause(from: &[&str]) -> String {
    if from.is_empty() {
        String::new()
    } else {
        format!("; --from join fields: {}", from.join(", "))
    }
}

const LINEAR_RAW: &str = "Raw Linear GraphQL data; foac adds no envelope";

pub(crate) fn linear_list(key: &str, from: &[&str]) -> String {
    section(&[
        &format!(
            r#"{{"{key}": {{"nodes": [<record>, ...], "pageInfo": {{"hasNextPage": true, "endCursor": "..."}}}}}}"#
        ),
        &format!("Records: {key}.nodes[]{}", join_clause(from)),
        &format!("Next page: pass {key}.pageInfo.endCursor to --after while hasNextPage is true"),
        LINEAR_RAW,
    ])
}

/// A connection nested one level down, under a parent object with sibling
/// scalars (`comment list`, `cycle list`).
pub(crate) fn linear_nested_list(parent: &str, key: &str, from: &[&str]) -> String {
    section(&[
        &format!(
            r#"{{"{parent}": {{..., "{key}": {{"nodes": [<record>, ...], "pageInfo": {{...}}}}}}}}"#
        ),
        &format!("Records: {parent}.{key}.nodes[]{}", join_clause(from)),
        &format!(
            "Next page: pass {parent}.{key}.pageInfo.endCursor to --after while hasNextPage is true"
        ),
        LINEAR_RAW,
    ])
}

pub(crate) fn linear_get(key: &str, id: &str) -> String {
    section(&[
        &format!(r#"{{"{key}": {{...}}}}"#),
        &format!("Primary identifier: {id}"),
        LINEAR_RAW,
    ])
}

pub(crate) fn linear_mutation(field: &str, key: &str) -> String {
    section(&[
        &format!(r#"{{"{field}": {{"success": true, "{key}": {{...}}}}}}"#),
        &format!("Primary identifier: {field}.{key}.id"),
        LINEAR_RAW,
    ])
}

pub(crate) fn linear_delete(field: &str) -> String {
    section(&[
        &format!(r#"{{"{field}": {{"success": true}}}}"#),
        LINEAR_RAW,
    ])
}

/// How one REST provider's `pageInfo` looks and how to fetch the next page.
pub(crate) struct Pagination {
    example: &'static str,
    next: &'static str,
}

pub(crate) const GITHUB_PAGES: Pagination = Pagination {
    example: r#"{"hasNextPage": true, "nextPage": 2, "hasPreviousPage": false, "previousPage": null}"#,
    next: "Next page: pass pageInfo.nextPage to --page while hasNextPage is true",
};

pub(crate) const SENTRY_CURSOR: Pagination = Pagination {
    example: r#"{"hasNextPage": true, "nextCursor": "...", "hasPreviousPage": false, "previousCursor": "..."}"#,
    next: "Next page: pass pageInfo.nextCursor to --cursor while hasNextPage is true",
};

pub(crate) const END_CURSOR: Pagination = Pagination {
    example: r#"{"hasNextPage": true, "endCursor": "..."}"#,
    next: "Next page: pass pageInfo.endCursor to --after while hasNextPage is true",
};

pub(crate) const NEXT_START_AT: Pagination = Pagination {
    example: r#"{"hasNextPage": true, "nextStartAt": 25}"#,
    next: "Next page: pass pageInfo.nextStartAt to --start-at while hasNextPage is true",
};

pub(crate) const NEXT_PAGE_TOKEN: Pagination = Pagination {
    example: r#"{"hasNextPage": true, "nextPageToken": "..."}"#,
    next: "Next page: pass pageInfo.nextPageToken to --after while hasNextPage is true",
};

pub(crate) const SINGLE_PAGE: Pagination = Pagination {
    example: r#"{"hasNextPage": false}"#,
    next: "All results are returned in one page",
};

pub(crate) const NEON_SINGLE_PAGE: Pagination = Pagination {
    example: r#"{"hasNextPage": false, "endCursor": null}"#,
    next: "All results are returned in one page",
};

pub(crate) fn rest_list(items: &str, from: &[&str], page: &Pagination) -> String {
    section(&[
        &format!(
            r#"{{"items": [<record>, ...], "pageInfo": {}}}"#,
            page.example
        ),
        &format!("Records: items[], {items}{}", join_clause(from)),
        page.next,
        "The items/pageInfo envelope is foac's; each item is unmodified provider data",
    ])
}

/// A REST get or mutation response: one raw provider object. An empty `id`
/// omits the identifier line.
pub(crate) fn rest_obj(what: &str, id: &str) -> String {
    let envelope = format!("A single {what}: unmodified provider data, no foac envelope");
    if id.is_empty() {
        section(&[&envelope])
    } else {
        section(&[&envelope, &format!("Primary identifier: {id}")])
    }
}

pub(crate) fn rest_delete() -> String {
    section(&[
        "{} on success: the provider returns no response body, so foac prints an empty JSON object",
    ])
}

/// A non-list Slack response: the whole `ok` envelope, unmodified.
pub(crate) fn slack_ok(record: &str, id: &str) -> String {
    let envelope =
        format!(r#"{{"ok": true, "{record}": {{...}}}}, the raw Slack envelope, unmodified"#);
    if id.is_empty() {
        section(&[&envelope])
    } else {
        section(&[&envelope, &format!("Primary identifier: {id}")])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_matches_claps_section_style() {
        let styles = clap::builder::Styles::default();
        let style = styles.get_header();
        assert_eq!(
            header(),
            format!("{}Output:{}", style.render(), style.render_reset())
        );
    }

    #[test]
    fn linear_list_names_records_and_pagination_paths() {
        assert_eq!(
            linear_list("users", &["id", "name", "email"]),
            header()
                + "\n  \
             {\"users\": {\"nodes\": [<record>, ...], \"pageInfo\": {\"hasNextPage\": true, \"endCursor\": \"...\"}}}\n  \
             Records: users.nodes[]; --from join fields: id, name, email\n  \
             Next page: pass users.pageInfo.endCursor to --after while hasNextPage is true\n  \
             Raw Linear GraphQL data; foac adds no envelope"
        );
    }

    #[test]
    fn linear_nested_list_prefixes_paths_with_the_parent() {
        let help = linear_nested_list("issue", "comments", &["id"]);
        assert!(help.contains("issue.comments.nodes[]"));
        assert!(help.contains("issue.comments.pageInfo.endCursor"));
    }

    #[test]
    fn linear_get_and_mutations_name_the_envelope() {
        assert_eq!(
            linear_get("issue", "issue.id (UUID)"),
            header()
                + "\n  {\"issue\": {...}}\n  Primary identifier: issue.id (UUID)\n  \
             Raw Linear GraphQL data; foac adds no envelope"
        );
        let create = linear_mutation("issueCreate", "issue");
        assert!(create.contains("{\"issueCreate\": {\"success\": true, \"issue\": {...}}}"));
        assert!(create.contains("Primary identifier: issueCreate.issue.id"));
        assert_eq!(
            linear_delete("commentDelete"),
            header()
                + "\n  {\"commentDelete\": {\"success\": true}}\n  \
             Raw Linear GraphQL data; foac adds no envelope"
        );
    }

    #[test]
    fn rest_list_names_the_wrapper_and_the_next_page_flag() {
        assert_eq!(
            rest_list("raw GitHub issue objects", &["number"], &GITHUB_PAGES),
            header()
                + "\n  \
             {\"items\": [<record>, ...], \"pageInfo\": {\"hasNextPage\": true, \"nextPage\": 2, \"hasPreviousPage\": false, \"previousPage\": null}}\n  \
             Records: items[], raw GitHub issue objects; --from join fields: number\n  \
             Next page: pass pageInfo.nextPage to --page while hasNextPage is true\n  \
             The items/pageInfo envelope is foac's; each item is unmodified provider data"
        );
    }

    #[test]
    fn rest_obj_omits_the_identifier_line_when_empty() {
        assert_eq!(
            rest_obj("raw Vercel domain configuration object", ""),
            header()
                + "\n  A single raw Vercel domain configuration object: \
             unmodified provider data, no foac envelope"
        );
        assert!(rest_obj("raw GitHub release object", "id").contains("Primary identifier: id"));
    }

    #[test]
    fn rest_delete_documents_the_empty_object() {
        assert!(rest_delete().contains("{} on success"));
    }

    #[test]
    fn slack_ok_shows_the_envelope() {
        assert_eq!(
            slack_ok("user", "user.id; email at user.profile.email"),
            header()
                + "\n  {\"ok\": true, \"user\": {...}}, the raw Slack envelope, unmodified\n  \
             Primary identifier: user.id; email at user.profile.email"
        );
    }
}
