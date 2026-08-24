# foac

foac, the Father Of All CLIs: one CLI for all your SaaS providers (Linear,
GitHub, Sentry, Slack, and more on the way), built for the coding agents on
your machine rather than for you. Install it once, log in once, and every
harness (Claude Code, Cursor, Codex, Gemini CLI, ...) can use all your
providers without any setup of its own. Humans at a TTY
get readable tables from the same commands.

If your agents spend their context loading MCP tool catalogs and fetching API
docs, foac replaces all of that with a single binary that already knows the
APIs.

## Why harnesses like foac

- **Install once, auth once, share everywhere.** `foac auth <provider> login`
  validates and stores your token. Every harness and script on the machine
  reuses the same auth and the same provider settings, so a new harness needs
  no configuration at all. See [doc/auth.md](doc/auth.md).
- **Discovery is instant and offline.** foac is a local cache of your
  providers' APIs. Every command tree is compiled into the binary, so an
  agent learns an API through `--help` without a network round-trip or a doc
  lookup, and without spending tokens on either.
- **A grammar agents can guess.** An MCP server pushes its whole tool catalog
  into the context before the first call. foac has one consistent hierarchy,
  `foac <provider> <resource> <verb>`, and every provider follows it, so an
  agent discovers commands progressively and only reads what the task needs.
  Knowing `foac linear user list` exists is knowing `foac slack user list`
  exists.
- **The CLI shrinks and grows with what you enable.** Disabled or
  unauthenticated providers disappear from `--help` and from the installed
  agent skills, so they never take up context. Toggle providers globally or
  per project without touching auth: each project can run with a different
  set of providers, and re-enabling one never asks you to log in again.
- **Composable.** Compact JSON on stdout, errors as JSON on stderr with exit
  code 1, so providers pipe into each other:

  ```sh
  foac linear user list | jq -r '.users.nodes[].email' | xargs -n1 foac slack user get
  ```

- **Responses are the provider's raw JSON.** foac does not reshape what an
  API returns, so the upstream API docs describe foac's output too. Every
  list command paginates the same way, with the same flags, across all
  providers.
- **Self-installing agent skills.** `foac skill install` writes one skill per
  active provider for Claude Code and every agent reading
  `~/.agents/skills/`, and removes the skills of inactive providers. See
  [doc/agent-skills.md](doc/agent-skills.md).
- **Self-updating.** `foac update` pulls the latest release for your platform
  and refreshes any installed foac skills.

## Providers

| Provider | Covers | Docs |
| --- | --- | --- |
| Linear | Issues, projects, teams, users, cycles, labels, workflow states, documents, initiatives, milestones, status updates, attachments | [doc/linear.md](doc/linear.md) |
| GitHub | Repositories, issues, pull requests, reviews, Actions, branches, commits, checks, releases, labels, artifacts, collaborators | [doc/github.md](doc/github.md) |
| Sentry | Organizations, projects, issues, error events, releases | [doc/sentry.md](doc/sentry.md) |
| Slack | Conversations, messages, threads, users, message search, reactions | [doc/slack.md](doc/slack.md) |

Candidates for more providers are tracked in
[GitHub issues](https://github.com/lra/foac/issues).

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

## Quick start

```sh
foac auth linear login                                # once, for every harness
foac skill install                                    # teach your agents foac
foac linear issue list --team ENG --state "In Progress"
foac provider disable slack                           # auth stays; re-enable anytime
foac update
```

`foac update` downloads the latest GitHub release for this platform, replaces
the running binary, and refreshes any foac provider skills already installed.

foac stores editable provider settings in
`~/.config/foac/config.toml` and machine-managed credentials in
`~/.config/foac/credentials.json` (or the equivalent paths under
`XDG_CONFIG_HOME`). The credentials file is atomically replaced and kept mode
`0600` on Unix. Legacy `config.json` files are intentionally ignored and are
not migrated or deleted.

To toggle providers per project, drop a `.foac.toml` in the project folder:

```toml
enabled_providers = ["linear"]   # on here even if disabled globally
disabled_providers = ["slack"]   # off here even if enabled globally
```

foac uses the nearest `.foac.toml` found from the working directory up to `/`;
its toggles override the global ones, and auth is never affected.
`foac provider <enable|disable> <name> --local` edits that nearest file for
you, creating `./.foac.toml` when none exists.

Other commands check GitHub for a newer release at most once a day, and print a notice on stderr while one exists. They never auto-install. Set `FOAC_NO_UPDATE_CHECK` (or `CI`) to skip the check.

Humans get auto-rendered tables at an interactive terminal instead of JSON;
see [doc/output.md](doc/output.md).
