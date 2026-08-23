---
name: foac
description: Use the foac CLI to interact with Linear and GitHub from the shell. Linear covers issues, projects, teams, users, cycles, labels, workflow states, documents, initiatives, milestones, status updates, and attachments. GitHub covers repositories, issues, pull requests, reviews, Actions, branches, commits, checks, releases, labels, artifacts, and collaborators.
---

# foac

foac wraps external service APIs as CLI subcommands, designed for LLM agents:
every command prints compact JSON on stdout, errors go to stderr with exit
code 1, and nothing is interactive.

## Structure

```text
foac <provider> <resource> <verb> [flags]
```

- Providers: `linear`, `github`.
- Resources are nouns (`issue`, `project`, `team`, `user`, ...), verbs are
  `list`, `get`, `create`, `update`, `delete`.
- `--help` at any level is the ground truth for what exists and which flags
  each verb takes. Explore with `foac linear --help`, then
  `foac linear <resource> --help`; use the same pattern for GitHub.

## Conventions

- **Auth**: Linear needs `LINEAR_API_KEY`. GitHub uses `GITHUB_TOKEN`, then
  falls back to `gh auth token` when the GitHub CLI is authenticated.
- **GitHub permissions**: classic tokens need `repo` for private repositories.
  Fine-grained tokens need Metadata read plus read or write access, as used, to
  Issues, Pull requests, Actions, Checks, Commit statuses, Contents, and
  Administration. Branch protection and collaborator changes need
  Administration write.
- **Output**: the raw API response as JSON on stdout. Failures print the
  API's error JSON on stderr and exit non-zero.
- **Linear pagination**: `list` verbs take `--limit N` (default 50) and
  `--after CURSOR`; loop using `pageInfo.endCursor` while `hasNextPage` is true.
- **GitHub pagination**: `list` verbs take `--limit N` (default 50, maximum
  100) and `--page N`; output is `{"items":[...],"pageInfo":{...}}`. Follow
  `nextPage` while `hasNextPage` is true.
- **Linear filters accept names**: filter flags on `list` verbs take a UUID or a
  human value — a team key (`ENG`), a state name (`In Progress`), a user
  email or display name, a project or label name.
- **Linear mutations need UUIDs**: flags on `create`/`update` verbs (`--assignee`,
  `--state`, `--project`, ...) require UUIDs. Look them up first with the
  matching `list` command. Issues are the exception: `get`, `update`, and
  `--issue` flags accept an identifier like `ENG-123` as well.
- **Updates are partial**: `update` verbs only change the flags you pass;
  omitted flags keep their value. Fields cannot be cleared to null.
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

## Examples

```sh
foac linear issue list --team ENG --state "In Progress"
foac linear issue create --team <TEAM_UUID> --title "Fix login" --description "..."
foac linear comment create --issue ENG-123 --body "Done, see PR #42"
foac github issue list --repo alephic-ai/example --state open
foac github pull create --repo alephic-ai/example --head feature --base main --title "Add feature" --body-file /tmp/pr.md
foac github review create --repo alephic-ai/example --pull 42 --event approve --body "Looks good"
foac github run rerun --repo alephic-ai/example 123456 --failed
```

## Maintenance

`foac update` replaces the binary with the latest release; `foac version`
prints the installed version. About once a day, foac may print a two-line
notice on stderr when a newer GitHub release exists — it never auto-installs,
and the notice is not JSON. Set `FOAC_NO_UPDATE_CHECK` to disable it.
Reinstall this skill with `foac skill install` after updating.
