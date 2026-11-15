# Output

Provider commands print the provider's response as compact JSON on stdout. At an
interactive terminal, foac renders it as a table sized to the terminal width
instead. Pick a format explicitly with `--format json|table|auto` or the
`FOAC_FORMAT` environment variable; pipes and CI (`CI` set) always get JSON.
`about`, `version`, `update`, and `skill` ignore `--format`.

JSON is the contract: it is the provider's data, unmodified. The table is a
reading view of that same data, built by shape heuristics in `src/output.rs`,
and it may change between releases. Never parse it.

## Tables

A list renders one row per record, then a footer with the row count:

```text
 identifier | title                 | priority | state       | assignee
------------+-----------------------+----------+-------------+----------
 ENG-123    | Fix the login timeout | 2        | In Progress | ada

1 row, 7 more columns (--format json shows all)
More results: endCursor=b439481e-60b2-4da2-abcf-28707c7dff22 (see --help for the paging flag)
```

- Columns are ordered for reading, not in the API's order: what names the
  record first (`identifier`, `number`, `key`, `name`, `title`, ...), then
  what it is triaged by (`state` or `status`, `priority`, `assignee`,
  `labels`, ...), then ordinary values, timestamps, opaque IDs, and URLs.
  Last come the fields that need a lookup to read: Jira's
  `customfield_10042` fields and its `expand` hint. The API's order is kept
  within each group. Field names match whatever their case, so an Airtable
  `Name` or `Status` field sorts as the name and the status, and a Notion
  page's title leads under whatever name its database gives it. The
  record's own `id` leads when nothing else names the row, or when it is a
  short string a command takes (a Slack `C...` ID, an Airtable `rec...` ID).
- `null` is a blank cell, and a column that is blank in every row is dropped.
- A nested record shows the field that names it (`state` renders as
  `In Progress`, `user` as its `login`), and a list of them is joined with
  commas. A Notion property shows its value rather than its typed wrapper:
  a `select` is its option's name, rich text is its plain text. A nested
  value with nothing naming it is left to JSON.
- When a row has more fields than the terminal fits (REST rows carry ~80),
  only the leading columns render and the footer counts the rest. Long text
  is cut with `…`; an ID is shown whole or not at all, since half an ID
  cannot be copied.
- The `More results` line appears only while there is a next page, and names
  the cursor to pass; the command's `--help` Output section says which flag
  takes it.
- An empty list prints `No results.`

A single record (`get`, and the result of a `create` or `update`) renders as
`FIELD | VALUE` rows in the same order. Long text wraps and keeps its own
line breaks, so a description or a comment body is readable in full. Fields
with no value are counted under the table instead of listed.

Jira issues and Airtable records keep their data under a `fields` key, and
Notion pages under `properties`; table mode lifts those fields so the row
shows the record, not its envelope.

Color (bold headers, a dimmed footer) is used at a terminal and turned off by
a non-empty `NO_COLOR`.

## Errors

A failed command prints its error on stderr and exits with code 1. A
provider's error is its JSON body, unmodified. In table mode a one-line
`error:` headline carrying the body's message comes first:

```text
error: Not Found
{"message":"Not Found","documentation_url":"https://docs.github.com/rest/issues/issues#get-an-issue","status":"404"}
```

With `--format json`, `FOAC_FORMAT=json`, a pipe, or CI, the headline is
omitted and stderr holds only the error.
