---
<!-- foac-provider:github -->
name: foac-github
description: Use the foac CLI to interact with GitHub from the shell. Covers repositories, issues, pull requests, reviews, Actions, branches, commits, checks, releases, labels, artifacts, and collaborators.
<!-- /foac-provider:github -->
<!-- foac-provider:linear -->
name: foac-linear
description: Use the foac CLI to interact with Linear from the shell. Covers issues, projects, teams, users, cycles, labels, workflow states, documents, initiatives, milestones, status updates, and attachments.
<!-- /foac-provider:linear -->
<!-- foac-provider:sentry -->
name: foac-sentry
description: Use the foac CLI to interact with Sentry from the shell. Covers organizations, projects, issues, error events, and releases.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
name: foac-slack
description: Use the foac CLI to interact with Slack from the shell. Covers conversations, messages, threads, users, message search, and reactions.
<!-- /foac-provider:slack -->
---

<!-- rumdl-disable MD022 MD025 -->
<!-- foac-provider:github -->
# foac-github
<!-- /foac-provider:github -->
<!-- foac-provider:linear -->
# foac-linear
<!-- /foac-provider:linear -->
<!-- foac-provider:sentry -->
# foac-sentry
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
# foac-slack
<!-- /foac-provider:slack -->
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
  The top-level `--help` lists only authenticated, enabled providers.
<!-- foac-provider:linear -->
- `linear`: issues, projects, teams, users, cycles, labels, workflow states,
  documents, initiatives, milestones, status updates, and attachments.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- `github`: repositories, issues, pull requests, reviews, Actions, branches,
  commits, checks, releases, labels, artifacts, and collaborators.
<!-- /foac-provider:github -->
<!-- foac-provider:sentry -->
- `sentry`: organizations, projects, issues, error events, and releases.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
- `slack`: conversations, messages, threads, users, message search, and reactions.
<!-- /foac-provider:slack -->
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
- **Provider toggles**: `foac provider <enable|disable> <name>` turns a provider
  on or off (state kept in `~/.config/foac/config.toml`) and prints the same
  per-provider enabled map as `foac provider list`. Disabled providers are
  hidden from discovery and their commands refuse to run. A `.foac.toml` file
  with `enabled_providers` and/or `disabled_providers` string arrays, found in
  the working directory or the nearest parent, overrides the global toggles
  for that folder tree. Add `--local` to enable/disable to write the toggle to
  that nearest `.foac.toml` instead (created in the working directory if none
  exists).
- **Storage**: Credentials are pretty-printed in
  `~/.config/foac/credentials.json`, atomically replaced, and mode `0600`
  before secret bytes are written on Unix. Settings use comment-preserving
  TOML. Missing files are valid first-run state; malformed stores fail closed
  independently with their path and cause. Legacy `config.json` is ignored and
  is not migrated or deleted.
<!-- foac-provider:linear -->
- **Linear auth precedence**: `LINEAR_API_KEY`, then the credentials file.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- **GitHub auth precedence**: `GITHUB_TOKEN`, then the credentials file, then
  `gh auth token`.
<!-- /foac-provider:github -->
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
- **Auth status**: Status commands perform live validation and print
  `authenticated`, `unauthenticated`, or `error`, including the
  credential source and safe account identity when available. `foac auth status`
  prints an object keyed by provider; `foac auth <provider> <status|login|logout>`
  prints a one-key map for that provider (login matches status fields; logout
  reports `removed`). Agents should parse that JSON (`--format json`). A TTY
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
  Failures print
  the API's error JSON on stderr and exit non-zero.
<!-- foac-provider:linear -->
- **Linear pagination**: `list` verbs take `--limit N` (default 50) and
  `--after CURSOR`; loop using `pageInfo.endCursor` while `hasNextPage` is true.
- **Linear filters accept names**: filter flags on `list` verbs take a UUID or a
  human value: a team key (`ENG`), a state name (`In Progress`), a user
  email or display name, a project or label name.
- **Linear mutations need UUIDs**: flags on `create`/`update` verbs (`--assignee`,
  `--state`, `--project`, ...) require UUIDs. Look them up first with the
  matching `list` command. Issues are the exception: `get`, `update`, and
  `--issue` flags accept an identifier like `ENG-123` as well.
- **Updates are partial**: `update` verbs only change the flags you pass;
  omitted flags keep their value. Fields cannot be cleared to null.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- **GitHub pagination**: `list` verbs take `--limit N` (default 50, maximum
  100) and `--page N`; output is `{"items":[...],"pageInfo":{...}}`. Follow
  `nextPage` while `hasNextPage` is true.
- **GitHub repositories**: pass `--repo OWNER/NAME` anywhere after `github`,
  or omit it inside a git checkout whose `origin` (or another remote) points to
  github.com.
- **GitHub identifiers**: commands use GitHub numbers, database IDs, usernames,
  refs, names, or file names as described by their help. `release get` requires
  an explicit `--id` or `--tag`, so numeric tags remain unambiguous.
- **Long Markdown**: GitHub commands accept mutually exclusive `--body` and
  `--body-file`. Nested API structures use native JSON flags such as
  `--comments-json`, `--inputs-json`, and `--rules-json`.
- **Metadata only**: GitHub release assets, Actions artifacts, and run jobs are
  JSON metadata. Binary uploads/downloads and log streaming are not supported.
<!-- /foac-provider:github -->
<!-- foac-provider:sentry -->
- **Sentry organization**: pass `--org SLUG` anywhere after `sentry`, or set
  `SENTRY_ORG`; only `org list` works without it. On a TTY,
  `foac auth sentry login` first asks for the Sentry hostname (default
  `sentry.io`, always https) and saves it; with redirected stdin it reads only
  the token, so pass `--host HOSTNAME` to save a self-hosted instance
  non-interactively. `SENTRY_URL` overrides the saved host.
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

## Examples

```sh
foac auth status
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

## Maintenance

`foac skill install` and `foac update` report byte-identical skills as
`Unchanged` without rewriting them. `foac update` replaces the binary with the
latest release and refreshes any foac provider skills already installed in
`~/.claude/skills` or `~/.agents/skills`; `foac version` prints the installed
version. foac checks
GitHub at most once a day and prints a two-line notice on stderr while a newer
release exists. It never auto-installs, and the notice is not JSON. Set
`FOAC_NO_UPDATE_CHECK` (or `CI`) to disable it.
