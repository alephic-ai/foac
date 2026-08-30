---
<!-- foac-provider:confluence -->
name: foac-confluence
description: Use the foac CLI to interact with Confluence from the shell. Covers spaces, pages, footer comments, and CQL search.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
name: foac-firecrawl
description: Use the foac CLI to interact with Firecrawl from the shell. Covers web scraping, site maps, web search, crawl jobs, batch scrapes, browsing agents, and team usage.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:github -->
name: foac-github
description: Use the foac CLI to interact with GitHub from the shell. Covers repositories, issues, pull requests, reviews, Actions, branches, commits, checks, releases, labels, artifacts, and collaborators.
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
name: foac-jira
description: Use the foac CLI to interact with Jira from the shell. Covers issues, comments, projects, sprints, users, and workflow transitions.
<!-- /foac-provider:jira -->
<!-- foac-provider:linear -->
name: foac-linear
description: Use the foac CLI to interact with Linear from the shell. Covers issues, projects, teams, users, cycles, labels, workflow states, documents, initiatives, milestones, status updates, and attachments.
<!-- /foac-provider:linear -->
<!-- foac-provider:neon -->
name: foac-neon
description: Use the foac CLI to interact with Neon from the shell. Covers organizations, projects, branches, databases, roles, compute endpoints, operations, and connection URIs.
<!-- /foac-provider:neon -->
<!-- foac-provider:sentry -->
name: foac-sentry
description: Use the foac CLI to interact with Sentry from the shell. Covers organizations, projects, issues, error events, and releases.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
name: foac-slack
description: Use the foac CLI to interact with Slack from the shell. Covers conversations, messages, threads, users, message search, and reactions.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
name: foac-vercel
description: Use the foac CLI to interact with Vercel from the shell. Covers teams, projects, deployments, account domains, and project domains.
<!-- /foac-provider:vercel -->
---

<!-- rumdl-disable MD022 MD025 -->
<!-- foac-provider:confluence -->
# foac-confluence
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
# foac-firecrawl
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:github -->
# foac-github
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
# foac-jira
<!-- /foac-provider:jira -->
<!-- foac-provider:linear -->
# foac-linear
<!-- /foac-provider:linear -->
<!-- foac-provider:neon -->
# foac-neon
<!-- /foac-provider:neon -->
<!-- foac-provider:sentry -->
# foac-sentry
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
# foac-slack
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
# foac-vercel
<!-- /foac-provider:vercel -->
<!-- rumdl-enable MD022 MD025 -->

foac wraps external provider APIs as CLI subcommands for LLM agents: every
command prints compact JSON on stdout (pass `--format json` when parsing; a
human at an interactive terminal gets a table instead), errors go to stderr
with exit code 1, and provider API commands are non-interactive. Auth login is
the explicit exception: it securely prompts on a TTY or reads a token from
redirected stdin.

## Structure

```text
foac <provider> <resource> <verb> [flags]
```

- A provider is the external product or API named by the first command segment.
  The top-level `--help` lists only authenticated, enabled providers, under a
  separate `Providers:` heading.
<!-- foac-provider:linear -->
- `linear`: issues, projects, teams, users, cycles, labels, workflow states,
  documents, initiatives, milestones, status updates, and attachments.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- `github`: repositories, issues, pull requests, reviews, Actions, branches,
  commits, checks, releases, labels, artifacts, and collaborators.
<!-- /foac-provider:github -->
<!-- foac-provider:confluence -->
- `confluence`: spaces, pages, footer comments, and CQL search.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
- `firecrawl`: web scraping, site maps, web search, crawl jobs, batch
  scrapes, browsing agents, and team usage.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:jira -->
- `jira`: issues, comments, projects, sprints, users, and workflow
  transitions.
<!-- /foac-provider:jira -->
<!-- foac-provider:neon -->
- `neon`: organizations, projects, branches, databases, roles, compute
  endpoints, operations, and connection URIs.
<!-- /foac-provider:neon -->
<!-- foac-provider:sentry -->
- `sentry`: organizations, projects, issues, error events, and releases.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
- `slack`: conversations, messages, threads, users, message search, and
  reactions.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
- `vercel`: teams, projects, deployments, account domains, and project domains.
<!-- /foac-provider:vercel -->
- Resources are nouns (`issue`, `project`, `team`, `user`, ...), verbs are
  `list`, `get`, `create`, `update`, `delete`.
- `--help` at any level lists what exists and which flags each verb takes.
  Explore with `foac <provider> --help`, then
  `foac <provider> <resource> --help`.

## Conventions

- **Auth commands**: Use `foac auth status` for all providers, or
  `foac auth <provider> <status|login|logout>` for one. Login securely reads and
  validates a token before saving it in foac's credentials file; when stdin is
  redirected, it reads the token from stdin. Logout removes only foac's stored
  credential. Use `foac auth --help` to list auth targets.
- **Instances**: a provider can hold several named logins to different
  tenants (e.g. two Slack workspaces). `foac auth <provider> login --instance
  <name>` stores one; commands select it with the global `-i`/`--instance`
  flag, else the nearest `.foac.toml` (or global config) `[defaults]` table
  (`slack = "workb"`), else the `default`
  instance — which is the unnamed login and behaves exactly as before.
  Environment tokens and the `gh` fallback apply to the default instance
  only; a named instance uses exactly its stored credentials. Instance names
  are lowercase letters, digits, `-`, `_`.
- **Provider toggles**: `foac provider <enable|disable> <name>` turns a provider
  on or off (state kept in `~/.config/foac/config.toml`) and prints the same
  per-provider map as `foac provider list`, which reports each provider's
  `enabled`, `authenticated` (a credential resolves; not validated against the
  API), and `skill_installed` state, plus one `provider@instance` entry per
  stored named instance. Add `--instance <name>` to toggle a single instance
  (stored as `provider@instance` in the same arrays) instead of the whole
  provider. Disabled providers and instances are hidden from discovery and
  their commands refuse to run. A `.foac.toml` file
  with `enabled_providers` and/or `disabled_providers` string arrays, found in
  the working directory or the nearest parent, overrides the global toggles
  for that folder tree. Add `--local` to enable/disable to write the toggle to
  that nearest `.foac.toml` instead (created in the working directory if none
  exists).
- **Storage**: Credentials are pretty-printed in
  `~/.config/foac/credentials.json` (nested provider → instance → fields),
  atomically replaced, and mode `0600`
  before secret bytes are written on Unix. Settings use comment-preserving
  TOML. Missing files are valid first-run state; malformed stores fail closed
  independently with their path and cause.
<!-- foac-provider:linear -->
- **Linear auth precedence**: `LINEAR_API_KEY`, then the credentials file.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- **GitHub auth precedence**: `GITHUB_TOKEN`, then the credentials file, then
  `gh auth token`.
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
- **Jira auth**: every command needs an Atlassian host, email, and API token.
  Each resolves independently: `--host`/`--email` flags, then
  `ATLASSIAN_HOST`/`ATLASSIAN_EMAIL`/`ATLASSIAN_API_TOKEN`, then the
  `atlassian` credentials saved by `foac auth jira login` (which prompts for
  all three, or reads one line per missing value from redirected stdin in
  host, email, token order). A token that is neither in the environment nor
  stored is read from redirected stdin, so it never has to appear in shell
  history. The stored credential is shared at the Atlassian vendor level:
  logging in or out through Jira or Confluence covers both.
<!-- /foac-provider:jira -->
<!-- foac-provider:confluence -->
- **Confluence auth**: every command needs an Atlassian host, email, and API
  token. Each resolves independently: `--host`/`--email` flags, then
  `ATLASSIAN_HOST`/`ATLASSIAN_EMAIL`/`ATLASSIAN_API_TOKEN`, then the
  `atlassian` credentials saved by `foac auth confluence login` (which prompts
  for all three, or reads one line per missing value from redirected stdin in
  host, email, token order). A token that is neither in the environment nor
  stored is read from redirected stdin, so it never has to appear in shell
  history. The stored credential is shared at the Atlassian vendor level:
  logging in or out through Jira or Confluence covers both.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
- **Firecrawl auth precedence**: `FIRECRAWL_API_KEY`, then the credentials
  file. On a TTY, `foac auth firecrawl login` first asks for the host
  (default `api.firecrawl.dev`; an explicit `http://` scheme is kept, so a
  local Docker deployment works) and saves it with the instance's
  credentials; with redirected stdin it reads only the token, so pass
  `--host URL` to save a self-hosted host non-interactively. A self-hosted
  Firecrawl with authentication disabled accepts any non-empty token.
  `FIRECRAWL_API_URL` overrides the saved host for the default instance
  only.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:neon -->
- **Neon auth precedence**: `NEON_API_KEY`, then the credentials file.
<!-- /foac-provider:neon -->
<!-- foac-provider:sentry -->
- **Sentry auth precedence**: `SENTRY_AUTH_TOKEN`, then the credentials file.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
- **Slack auth capabilities**: ordinary commands prefer `SLACK_BOT_TOKEN`
  (`xoxb-`), then the bot credential in the credentials file, then
  `SLACK_USER_TOKEN` (`xoxp-`), then the stored user credential. `slack search`
  uses user env then stored user and requires `search:read`. Bot-only setups
  cannot search; user-only setups can use every command as the installing user;
  when both exist, ordinary commands run as the bot and search runs as the user.
  With neither, Slack is inactive. `foac auth slack login` prompts for bot then
  user, validates both before storing either, and allows either to be blank.
  Before prompting, it links to Slack's app management page and prints a JSON
  app manifest with the recommended bot and user scopes. Redirected input is
  two lines in the same order. Slack logout removes both.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
- **Vercel auth precedence**: `VERCEL_TOKEN`, then the credentials file.
<!-- /foac-provider:vercel -->
- **Auth status**: Status commands perform live validation and print
  `authenticated`, `unauthenticated`, or `error`, including the
  credential source and safe account identity when available. `foac auth status`
  prints an object keyed by provider, plus one `provider@instance` entry per
  stored named instance; `foac auth <provider> <status|login|logout>`
  prints a one-key map for that provider and instance (login matches status
  fields; logout reports `removed`). Agents should parse that JSON
  (`--format json`). A TTY
  shows a short summary for the single-provider commands; the all-provider
  table flattens `account` to an identity string. They exit zero after printing
  the report; inspect the JSON status values.
<!-- foac-provider:github -->
- **GitHub permissions**: classic tokens need `repo` for private repositories.
  Fine-grained tokens need Metadata read plus read or write access, as used, to
  Issues, Pull requests, Actions, Checks, Commit statuses, Contents, and
  Administration. Branch protection and collaborator changes need
  Administration write.
<!-- /foac-provider:github -->
- **Output**: the raw API response as JSON on stdout. JSON
  success output (including `auth` and `provider`) renders as a table sized to
  the terminal when stdout is an interactive TTY and `CI` is not set, so agents
  parsing stdout must pass
  `--format json` or set `FOAC_FORMAT=json`; pipes and CI always get JSON.
  `version`, `update`, and `skill` ignore `--format`.
  Every provider command's `--help` ends with an Output section naming its
  envelope, record path, `--from` join fields, and pagination paths — check
  it before parsing a response you have not seen.
  Failures print
  the API's error JSON on stderr and exit non-zero.
- **Piped joins**: pipe one command's `list` output into a `get` verb that
  takes its identifier as a positional argument (every `get` except
  `github release get`, whose selector is `--id`/`--tag`) and omit that
  argument to run one get per list item, joining on the field named by
  `--from`:
  `foac <provider> <resource> list | foac <provider> <resource> get --from
  <field>` (the two commands may target different providers; dots in the
  field reach into nested objects, e.g. `--from profile.email`). Overlap,
  membership, or "who is on both" questions are this join — not two lists
  plus a script. Piped input that
  is not JSON is one value per line, so `grep` output composes too. Successes
  stream as one JSON document per get on stdout (a TTY renders them as one
  combined table, one row per result); values the API reports as missing are
  summarized in a single stderr line (`5 of 21 not found: ...`); any other
  failure (auth, rate limit, network) prints its error JSON on stderr as it
  happens. The exit code is 0 when at least one get succeeded and the rest
  were misses, 1 when all missed or any get failed outright.
<!-- foac-provider:linear -->
- **Linear pagination**: `list` verbs take `--limit N` (default 50) and
  `--after CURSOR`; loop using `pageInfo.endCursor` while `hasNextPage` is true.
- **Linear filters accept names**: filter flags on `list` verbs take a UUID or a
  human value: a team key (`ENG`), a state name (`In Progress`), a user
  email or display name, a project or label name.
- **Linear mutations need UUIDs**: flags on `create`/`update` verbs
  (`--assignee`, `--state`, `--project`, ...) require UUIDs. Look them up first
  with the matching `list` command. Issues are the exception: `get`, `update`,
  and `--issue` flags accept an identifier like `ENG-123` as well.
- **Updates are partial**: `update` verbs only change the flags you pass;
  omitted flags keep their value. Fields cannot be cleared to null.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- **GitHub pagination**: `list` verbs take `--limit N` (default 50, maximum
  100) and `--page N`; output is `{"items":[...],"pageInfo":{...}}`. Follow
  `nextPage` while `hasNextPage` is true.
- **GitHub repositories**: repository-scoped commands take `--repo OWNER/NAME`
  anywhere after the resource name (`issue`, `pull`, ...); omit it inside a git
  checkout whose `origin` (or another remote) points to github.com. `repo list`
  is account-wide and takes no `--repo`.
- **GitHub identifiers**: commands use GitHub numbers, database IDs, usernames,
  refs, names, or file names as described by their help. `release get` requires
  an explicit `--id` or `--tag`, so numeric tags remain unambiguous.
- **Long Markdown**: GitHub commands accept mutually exclusive `--body` and
  `--body-file`. Nested API structures use native JSON flags such as
  `--comments-json`, `--inputs-json`, and `--rules-json`. In `--rules-json`,
  `required_status_checks.contexts` is deprecated; use
  `required_status_checks.checks` with `{"context": ..., "app_id": ...}`.
- **Metadata only**: GitHub release assets, Actions artifacts, and run jobs are
  JSON metadata. Binary uploads/downloads and log streaming are not supported.
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
- **Jira pagination**: `issue list` takes `--limit N` and `--after TOKEN`;
  follow `pageInfo.nextPageToken` while `hasNextPage` is true. Other list
  verbs take `--limit N` and `--start-at N`; follow `pageInfo.nextStartAt`.
  Output is `{"items":[...],"pageInfo":{...}}`.
- **Jira identifiers**: issues use keys like `ENG-123`. Projects accept a key or
  numeric ID; issue types and priorities accept a name or numeric ID; assignees
  are account IDs (find them with `user list --query`); `--board` accepts a
  numeric ID or an exact board name. `issue list --jql` takes raw JQL; Jira
  rejects unbounded queries, so without `--jql` it defaults to
  `created >= -30d ORDER BY created DESC`. To change status, list options with
  `transition list --issue ENG-123`, then
  `issue transition ENG-123 --to <transition id, transition name, or destination status name>`.
- **Jira text**: issue descriptions and comments accept mutually exclusive
  `--body` and `--body-file` (plain text or Jira wiki markup).
<!-- /foac-provider:jira -->
<!-- foac-provider:confluence -->
- **Confluence pagination**: `space`, `page`, and `comment` lists take
  `--limit N` and an opaque `--after CURSOR`; follow `pageInfo.endCursor`
  while `hasNextPage` is true. `search` is offset-paged with `--limit N` and
  `--start-at N`; follow `pageInfo.nextStartAt`. Output is
  `{"items":[...],"pageInfo":{...}}`.
- **Confluence identifiers**: spaces accept a key like `ENG` or a numeric ID;
  pages and comments use numeric IDs (find pages with `page list --space` or
  `search --cql`). `search --cql` takes raw CQL such as
  `type = page AND text ~ "login"`.
- **Confluence text**: page and comment bodies are written as Confluence wiki
  markup via mutually exclusive `--body` and `--body-file`, and read back in
  the storage representation. `page update` and `comment update` fetch the
  current version internally and re-send omitted fields unchanged, so there is
  no version flag to manage.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
- **Firecrawl output**: `scrape` prints Firecrawl's raw
  `{"success": true, "data": {...}}` with one key per requested format
  (`data.markdown`, `data.links`, `data.json`, ...) plus `data.metadata`.
  `map`, `search`, and `crawl list` are single-page foac lists
  (`{"items":[...],"pageInfo":{"hasNextPage":false}}`); `search` flattens the
  requested sources into one list, web first. Every other response is raw
  Firecrawl JSON. Credits are spent per page scraped; check
  `team credit-usage` before large crawls.
- **Firecrawl scraping**: `scrape URL` returns markdown by default; pass
  `--formats markdown,html,rawHtml,links,images,screenshot,summary,branding`
  (comma-separated) for more, `--json-prompt`/`--json-schema[-file]` for
  structured extraction (the `json` format, the replacement for the retired
  extract endpoint), and `--only-main-content false` for the whole page.
  Pipe a `map` or `search` list into `scrape --from url` to scrape every
  result. The same per-page flags apply to `crawl create` and
  `batch-scrape create`.
- **Firecrawl jobs**: `crawl`, `batch-scrape`, and `agent` are asynchronous.
  `create` returns `{"id": ...}` at once; `get ID` reads the job's status
  and result (scraped pages in `data` for crawl and batch-scrape, the
  agent's answer in `data` for agent); `cancel ID` stops it. Crawl and
  batch-scrape also have `errors ID` (failed pages) and `get --skip N`: a
  status with a `next` URL has more pages, pass its `skip` value. Add
  `--wait [--poll-interval S] [--wait-timeout S]` to `create` to block until
  the job is `completed`, `failed`, or `cancelled` and print that final
  status with `id` added; a timed-out or failed wait reports the job ID on
  stderr so the job can still be fetched or cancelled.
- **Firecrawl agents**: `agent create "PROMPT" [--url URL]... [--schema JSON]
  [--max-credits N]` runs a browsing agent; results land in `data` once
  `completed`. Agents spend tokens (`team token-usage`) on top of credits.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:neon -->
- **Neon project**: pass `--project ID` anywhere after `neon`, or set
  `NEON_PROJECT_ID`; only `org list` and `project list` work without it. Neon
  requires an organization ID on `project list` when the account belongs to
  an organization: pass `--org ID` or set `NEON_ORG_ID`, finding IDs with
  `org list`.
- **Neon pagination**: `project list`, `branch list`, and `operation list`
  take `--limit N` (default 50) and an opaque `--after CURSOR`; output is
  `{"items":[...],"pageInfo":{...}}`. Follow `pageInfo.endCursor` while
  `hasNextPage` is true. Other lists are not paginated.
- **Neon identifiers**: branches use IDs like `br-...` and compute endpoints
  IDs like `ep-...`; find them with `branch list` and `endpoint list`.
  `connection-uri` requires `--database` and `--role` and prints a URI
  containing that role's password.
- **Neon operations are asynchronous**: `branch create|delete` and
  `endpoint start|suspend|restart` return `operations` still running; poll
  `operation get ID` until `status` is `finished` before using the result.
  `branch delete` refuses the default branch and branches with children.
<!-- /foac-provider:neon -->
<!-- foac-provider:sentry -->
- **Sentry organization**: pass `--org SLUG` anywhere after `sentry`, or set
  `SENTRY_ORG`; only `org list` works without it. `--org` and `--project`
  take a slug or a numeric ID; IDs survive renames. On a TTY,
  `foac auth sentry login` first asks for the Sentry hostname (default
  `sentry.io`, always https) and saves it; with redirected stdin it reads only
  the token, so pass `--host HOSTNAME` to save a self-hosted host
  non-interactively. The host is saved with the instance's credentials;
  `SENTRY_URL` overrides it for the default instance only and is also
  normalized to https.
- **Sentry pagination**: `list` verbs take `--cursor CURSOR`; output is
  `{"items":[...],"pageInfo":{...}}`. Follow `nextCursor` while `hasNextPage`
  is true.
- **Sentry issues**: `issue` and `--issue` accept a numeric issue ID or a
  short ID like `PROJ-123`. `issue list` searches the organization, or one
  project with `--project SLUG`; `--query` takes Sentry search syntax such as
  `is:unresolved release:1.2.0`. Releases are read-only; use `sentry-cli` to
  create releases and upload sourcemaps.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
- **Slack pagination**: list and search commands take `--limit N` (default 100)
  and `--after CURSOR`; output is
  `{"items":[...],"pageInfo":{"hasNextPage":...,"endCursor":...}}`.
  Follow `endCursor` while `hasNextPage` is true.
- **Slack names**: conversation arguments accept an ID or a channel name such
  as `#eng`; user get accepts an ID, `@name`, display name, or email. Name
  resolution pages through the visible workspace directory. Email lookup
  requires `users:read.email`.
- **Slack messages**: `message list/get/create` accept `--thread-ts`; list reads
  replies and create posts a reply. Message text uses mutually exclusive
  `--body` and `--body-file`. Update and delete work only on messages posted by
  the selected identity (the bot when available, otherwise the user). Pass
  reaction names with or without surrounding colons.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
- **Vercel scope**: omit `--team` for the token's personal account, or pass a
  team ID like `team_...` anywhere after `vercel`. `VERCEL_TEAM_ID` is the
  default. Find team IDs with `team list`.
- **Vercel pagination**: list verbs take `--limit N` (default 20) and
  `--after CURSOR`; output is `{"items":[...],"pageInfo":{...}}`. Follow
  `pageInfo.endCursor` while `hasNextPage` is true. Vercel cursors are usually
  millisecond timestamps, but pass them back unchanged.
- **Vercel identifiers**: projects accept an ID or name; deployments accept an
  ID (and `get` also accepts a URL); domains use their DNS name. Project
  updates change only supplied fields. Deployment creation/uploads, logs, DNS
  records, and environment variables are not covered.
<!-- /foac-provider:vercel -->

<!-- foac-provider:github -->
## GitHub resources

- Collaboration: `repo`, `issue`, `comment`, `pull`, `review`.
- Actions: `workflow`, `run`.
- Git and checks: `branch`, `ref`, `branch-protection`, `commit`,
  `commit-comment`, `status`, `check-run`, `check-suite`.
- Administration: `release`, `release-asset`, `artifact`, `label`,
  `collaborator`.

Use `foac github <resource> --help` for the available verbs and flags. GitHub
issue lists exclude pull requests even though the upstream issues endpoint
returns both. Review creation accepts inline comments as a GitHub-native JSON
array through `--comments-json`; omit `--event` to create a pending review,
then use `review submit`.
<!-- /foac-provider:github -->

<!-- foac-provider:jira -->
## Jira resources

- Issues: `issue list|get|create|update|transition` and
  `comment list|create|update|delete`.
- Structure: `project list|get` and `sprint list|get` (sprints need
  `--board`).
- Directory and workflow: `user list|get` and `transition list`.

Use `foac jira <resource> --help` for flags and required arguments. Issue
`update` changes only the supplied fields; `--label` replaces the full label
list.
<!-- /foac-provider:jira -->

<!-- foac-provider:confluence -->
## Confluence resources

- Spaces: `space list|get`.
- Pages: `page list|get|create|update|delete` (`create` takes `--space`,
  `--title`, and optional `--parent`).
- Footer comments: `comment list|create|update|delete` (`list` and `create`
  take `--page`).
- Discovery: `search --cql`.

Use `foac confluence <resource> --help` for flags and required arguments.
Inline comments, attachments, whiteboards, and databases are not covered.
<!-- /foac-provider:confluence -->

<!-- foac-provider:firecrawl -->
## Firecrawl resources

- Synchronous: `scrape URL`, `map URL`, `search QUERY`.
- Jobs: `crawl list|create|get|errors|cancel`,
  `batch-scrape create|get|errors|cancel`, `agent create|get|cancel`.
- Usage: `team credit-usage|token-usage|queue-status|concurrency`.

Use `foac firecrawl <resource> --help` for flags. Local file parsing,
monitors, browser sessions, and page interaction are not covered.
<!-- /foac-provider:firecrawl -->

<!-- foac-provider:slack -->
## Slack resources

- Conversations: `conversation list|get`.
- Messages: `message list|get|create|update|delete`, including threads.
- Directory and discovery: `user list|get` and `search`.
- Reactions: `reaction add|remove`.

Use `foac slack <resource> --help` for flags and required arguments. Typical bot
scopes are `channels:history`, `channels:read`, `chat:write`, `groups:history`,
`groups:read`, `im:history`, `im:read`, `mpim:history`, `mpim:read`,
`reactions:write`, `users:read`, and `users:read.email`; only grant scopes for
the commands the app needs. User-only operation needs equivalent user scopes;
search uses the user credential (environment or config), never the bot token.
<!-- /foac-provider:slack -->

<!-- foac-provider:vercel -->
## Vercel resources

- Scope discovery: `team list|get`.
- Projects: `project list|get|create|update|delete`.
- Deployments: `deployment list|get|cancel|delete`.
- Account domains: `domain list|get|config|create|delete`.
- Project domains: `project-domain list|get|create|update|delete|verify`.

Use `foac vercel <resource> --help` for flags and required arguments. Domain
ownership and assignment are separate: `domain` manages the account-level
domain, while `project-domain` assigns a domain to a project.
<!-- /foac-provider:vercel -->

## Examples

```sh
foac auth status
# cross-provider join (works for any list | any get pair):
foac linear user list | foac slack user get --from email
foac github repo list | foac vercel project get --from name
```

<!-- foac-provider:linear -->

```sh
foac linear issue list --team ENG --state "In Progress"
foac linear issue create --team <TEAM_UUID> --title "Fix login" --description "..."
foac linear comment create --issue ENG-123 --body "Done, see PR #42"
```

<!-- /foac-provider:linear -->

<!-- foac-provider:github -->

```sh
foac github issue list --repo owner/repo --state open
foac github pull create --repo owner/repo --head feature --base main --title "Add feature" --body-file /tmp/pr.md
foac github review create --repo owner/repo --pull 42 --event approve --body "Looks good"
foac github run rerun --repo owner/repo 123456 --failed
```

<!-- /foac-provider:github -->

<!-- foac-provider:jira -->

```sh
foac jira issue list --jql 'project = ENG AND statusCategory != Done'
foac jira issue create --project ENG --type Task --summary "Fix login" --body "Steps..."
foac jira issue transition ENG-123 --to "In Progress"
foac jira comment create --issue ENG-123 --body "Done, see PR #42"
```

<!-- /foac-provider:jira -->

<!-- foac-provider:confluence -->

```sh
foac confluence page list --space ENG
foac confluence page create --space ENG --title "Runbook" --body-file /tmp/runbook.wiki
foac confluence comment create --page 12345 --body "Updated, see PR #42"
foac confluence search --cql 'type = page AND text ~ "login"'
```

<!-- /foac-provider:confluence -->

<!-- foac-provider:firecrawl -->

```sh
foac firecrawl scrape https://docs.example.com/api --formats markdown,links
foac firecrawl map https://docs.example.com --search "authentication" | foac firecrawl scrape --from url
foac firecrawl search "rust async runtime comparison" --limit 5 --tbs qdr:y
foac firecrawl crawl create https://docs.example.com --limit 50 --include-paths "/docs/*" --wait
foac firecrawl agent create "List the pricing tiers and their monthly prices" --url https://example.com/pricing --wait
```

<!-- /foac-provider:firecrawl -->

<!-- foac-provider:neon -->

```sh
foac neon branch list --project proj-1
foac neon branch create --project proj-1 --name preview --parent br-main-123
foac neon endpoint suspend ep-123 --project proj-1
foac neon connection-uri --project proj-1 --database app --role app_owner
```

<!-- /foac-provider:neon -->

<!-- foac-provider:sentry -->

```sh
foac sentry issue list --org acme --project backend --query "is:unresolved"
foac sentry issue latest-event PROJ-123 --org acme
foac sentry issue update PROJ-123 --org acme --status resolved
```

<!-- /foac-provider:sentry -->

<!-- foac-provider:slack -->

```sh
foac slack conversation get '#eng'
foac slack message list '#eng' --limit 50
foac slack message create '#eng' --body "PR is up: https://github.com/owner/repo/pull/42"
foac slack message create '#eng' --thread-ts 1724432400.123456 --body-file /tmp/reply.md
foac slack search 'deployment in:eng' --sort timestamp --direction desc
foac slack reaction add '#eng' 1724432400.123456 eyes
```

<!-- /foac-provider:slack -->

<!-- foac-provider:vercel -->

```sh
foac vercel team list
foac vercel project list --team team_123 --search web
foac vercel deployment list --project web --state READY
foac vercel project-domain create --project web preview.example.com --git-branch preview
```

<!-- /foac-provider:vercel -->

## Maintenance

`foac skill install` and `foac update` report byte-identical skills as
`Unchanged` without rewriting them. `foac update` replaces the binary with the
latest release and refreshes any foac provider skills already installed in
`~/.claude/skills` or `~/.agents/skills` — on a Homebrew install it exits 1 and
points at `brew upgrade foac`. However the binary was upgraded, the new version
refreshes those installed skills on its first run, so they always match the
running CLI. `foac version` prints the installed
version; `foac about` prints the brand banner, version, and repository URL.
foac checks
GitHub at most once a day and prints a two-line notice on stderr while a newer
release exists. It never auto-installs, and the notice is not JSON. Set
`FOAC_NO_UPDATE_CHECK` (or `CI`) to disable it.
