# foac

foac, the Father Of All CLIs

foac provides a consistent CLI for external providers, organized as
`foac <provider> <resource> <verb>`. A provider is the external product or API
named by the first command segment, such as `linear` or `github`.

## Install

[ubi](https://github.com/houseabsolute/ubi) installs the `foac` binary for your platform from [GitHub Releases](https://github.com/lra/foac/releases).

### 1. Install ubi

On Linux, macOS, FreeBSD, and NetBSD:

```sh
curl --silent --location \
    https://raw.githubusercontent.com/houseabsolute/ubi/master/bootstrap/bootstrap-ubi.sh |
    sh
```

This command installs `ubi` into `$HOME/bin` for a normal user, or into `/usr/local/bin` for root.

On Windows, run this command in PowerShell. It installs `ubi.exe` into the current directory:

```powershell
powershell -exec bypass -c "Invoke-WebRequest -URI 'https://raw.githubusercontent.com/houseabsolute/ubi/master/bootstrap/bootstrap-ubi.ps1' -UseBasicParsing | Invoke-Expression"
```

### 2. Install foac

```sh
ubi --project lra/foac --in "$HOME/bin"
```

On Windows:

```powershell
.\ubi.exe --project lra/foac --in "$HOME\bin"
```

If `$HOME/bin` is not on your `PATH`, add it.

## Run

```sh
foac
foac --help
foac version
foac update
```

`foac update` downloads the latest GitHub release for this platform and replaces the running binary.

Other commands check GitHub for a newer release at most once a day, and print a notice on stderr while one exists. They never auto-install. Set `FOAC_NO_UPDATE_CHECK` (or `CI`) to skip the check.

## Output

Provider commands print the provider's response as compact JSON on
stdout. At an interactive terminal, foac renders it as a table sized to the
terminal width instead. Pick a format explicitly with `--format json|table|auto`
or the `FOAC_FORMAT` environment variable; pipes and CI (`CI` set) always get
JSON. Errors stay on stderr as JSON with exit code 1, and `auth`, `provider`,
`version`, `update`, and `skill` ignore `--format`.

## Authentication

foac can validate every provider at once or manage each provider separately:

```sh
foac auth status
foac auth linear status
foac auth linear login
foac auth linear logout
foac auth github status
foac auth github login
foac auth github logout
foac auth sentry status
foac auth sentry login
foac auth sentry logout
foac auth slack status
foac auth slack login
foac auth slack logout
```

`login` prints a link and permission guidance, securely prompts for a personal
API token, validates it, and stores it in foac's config file
(`~/.config/foac/config.json`, or under `XDG_CONFIG_HOME`), which foac keeps
readable by the owner only. Pipe a token to `login` for non-interactive use.
Slack login prompts for both bot and user tokens; for non-interactive use, pipe
two lines in that order (either line may be blank). It also links to Slack's app
management page and prints a ready-to-paste JSON app manifest with foac's
recommended bot and user scopes. Tokens are never printed.

Environment variables take precedence over stored credentials. GitHub also
falls back to `gh auth token` when neither `GITHUB_TOKEN` nor a stored foac
credential is available. `logout` removes only foac's stored credentials
(both bot and user credentials for Slack); it does not unset environment
variables, log out the `gh` CLI, or revoke tokens at the provider.

Status commands validate credentials with the provider and print JSON. The
all-provider command prints an object keyed by provider; provider-specific
commands print one status object. A provider's `status` is `authenticated`,
`unauthenticated`, or `error`, and includes the credential `source` and safe
account identity when available. Status commands exit zero after printing the
report, so callers should inspect the JSON status values.

## Linear

`foac linear` talks to [Linear's GraphQL API](https://linear.app/developers/graphql). It uses `LINEAR_API_KEY` or a credential saved by `foac auth linear login`. Every command prints JSON on stdout; list commands paginate with `--limit`/`--after` and include `pageInfo` in the output.

```sh
export LINEAR_API_KEY=lin_api_...
foac linear issue list --team ENG --state "In Progress"
foac linear issue create --team <TEAM_UUID> --title "Fix the flux capacitor"
foac linear --help
```

## GitHub

`foac github` talks to GitHub.com's REST API. It uses `GITHUB_TOKEN`, a credential
saved by `foac auth github login`, or `gh auth token`, in that order.
Repository-scoped commands accept `--repo OWNER/NAME`; without it, foac uses the
current checkout's GitHub remote.

For classic tokens, the `repo` scope covers private-repository commands. For
fine-grained tokens, grant Metadata read plus read or write access—matching the
commands you will use—to Issues, Pull requests, Actions, Checks, Commit
statuses, Contents, and Administration. Branch protection and collaborator
changes require Administration write access.

```sh
export GITHUB_TOKEN=github_pat_...
foac github issue list --repo owner/repo --state open
foac github pull get 14 --repo owner/repo
foac github run list --repo owner/repo --status failure
foac github --help
```

GitHub list commands print `{"items":[...],"pageInfo":{...}}` and accept
`--limit`/`--page`. Commands with long Markdown fields accept either `--body`
or `--body-file`. Asset and artifact commands return metadata only; binary
transfer and Actions log retrieval are unsupported.

## Sentry

`foac sentry` talks to [Sentry's REST API](https://docs.sentry.io/api/). It
uses `SENTRY_AUTH_TOKEN` or a credential saved by `foac auth sentry login`.
Pass `--org SLUG` or set `SENTRY_ORG`. At an interactive terminal,
`foac auth sentry login` first asks for the Sentry hostname (default
`sentry.io`, always https — enter your own for a self-hosted instance) and
saves it alongside the token; piped logins read only the token, so pass
`--host sentry.example.com` to save a self-hosted instance non-interactively.
`SENTRY_URL` overrides the saved host. Issue commands accept numeric IDs
or short IDs like `PROJ-123`.

```sh
export SENTRY_AUTH_TOKEN=sntrys_...
foac sentry issue list --org acme --project backend --query "is:unresolved"
foac sentry issue latest-event PROJ-123 --org acme
foac sentry --help
```

Sentry list commands print `{"items":[...],"pageInfo":{...}}` and paginate
with `--cursor` using `pageInfo.nextCursor`. Releases are read-only; release
creation and sourcemap upload stay with `sentry-cli`.

## Slack

`foac slack` talks to Slack's Web API and supports bot tokens, user tokens, or
both. Ordinary commands prefer `SLACK_BOT_TOKEN`, then a bot credential saved
by `foac auth slack login`, then `SLACK_USER_TOKEN`, then the stored user
credential. Message search prefers `SLACK_USER_TOKEN`, then the stored user
credential, because Slack's `search.messages` method does not accept bot tokens.
Conversation arguments accept IDs or names such as `#eng`; `user get` accepts
an ID, `@name`, display name, or email.

| Available credentials | Ordinary commands | Search |
| --- | --- | --- |
| Bot only | Run as the app's bot | Unavailable |
| User only | Run as the installing user | Run as the installing user |
| Bot and user | Run as the app's bot | Run as the installing user |
| Neither | Slack is hidden from authenticated discovery | Unavailable |

`foac auth slack login` securely prompts for both token types, validates every
supplied token before changing the config, and stores them independently; leave
either prompt blank if it is not needed. With redirected stdin, supply the bot
token on the first line and user token on the second. `foac auth slack status`
accepts either token and reports the selected account's `token_type`. In a
user-only setup, actions are limited by that member's visibility and granted
user scopes, and writes are attributed to that member.

```sh
export SLACK_BOT_TOKEN=xoxb-...
export SLACK_USER_TOKEN=xoxp-...
printf '%s\n%s\n' "$SLACK_BOT_TOKEN" "$SLACK_USER_TOKEN" | foac auth slack login
foac slack conversation list
foac slack message create '#eng' --body "PR is up"
foac slack message list '#eng' --thread-ts 1724432400.123456
foac slack search 'deployment in:eng'
foac slack --help
```

Slack list and search commands print `{"items":[...],"pageInfo":{...}}` and
paginate with `--limit`/`--after` using `pageInfo.endCursor`. Long message text
accepts either `--body` or `--body-file`.

## Agents

`foac skill print` prints an agent skill explaining how the CLI is structured
and its conventions. `foac skill install` writes it into the skill folders of the
agents found on the machine: `~/.claude/skills/` for Claude Code, and the
cross-agent standard `~/.agents/skills/` read by Cursor, Codex, Gemini CLI,
GitHub Copilot, OpenCode, Amp, and others.

## Hacking

```sh
cargo run -- --help
cargo test --locked
```

[AGENTS.md](AGENTS.md) has the code layout, the add-a-command recipe, and the
conventions.
