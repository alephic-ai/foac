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

Linear and GitHub commands print the provider's response as compact JSON on
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
```

`login` prints a link and permission guidance, securely prompts for a personal
API token, validates it, and stores it in the operating system's secret store:
Keychain Services on macOS, Credential Manager on Windows, and Secret Service
on Linux. Pipe a token to `login` for non-interactive use. Tokens are never
printed.

Environment variables take precedence over stored credentials. GitHub also
falls back to `gh auth token` when neither `GITHUB_TOKEN` nor a stored foac
credential is available. `logout` removes only foac's stored credential; it
does not unset environment variables, log out the `gh` CLI, or revoke the token
at the provider.

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
foac github issue list --repo lra/foac --state open
foac github pull get 14 --repo lra/foac
foac github run list --repo lra/foac --status failure
foac github --help
```

GitHub list commands print `{"items":[...],"pageInfo":{...}}` and accept
`--limit`/`--page`. Commands with long Markdown fields accept either `--body`
or `--body-file`. Asset and artifact commands return metadata only; binary
transfer and Actions log retrieval are unsupported.

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
