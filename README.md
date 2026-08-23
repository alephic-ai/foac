# foac

foac, the Father Of All CLIs

## Why foac

Every SaaS product ships its own CLI with its own grammar, auth story, and
output quirks. foac wraps them all behind one consistent grammar —
`foac <provider> <resource> <verb>` — built first for LLM agents working in a
shell, with humans at a TTY served by the same commands.

- **Discovery is offline and deterministic.** The command tree is compiled in,
  so an agent learns any provider's API through `--help` with no network
  round-trip. Providers that are disabled or unauthenticated are hidden.
- **Log in once, reuse everywhere.** `foac auth <provider> login` stores a
  validated token in an owner-only config file; every agent harness and script
  on the machine reuses it.
- **Responses are the provider's raw JSON.** foac never reshapes what the API
  returned, so upstream API docs remain valid documentation for foac's output.

## Features

- Uniform grammar across providers: provider/resource/verb, `--limit` plus
  cursor-or-page flags, `{"items":[...],"pageInfo":{...}}` lists, JSON errors
  on stderr with exit code 1.
- Compact JSON on stdout; auto-rendered tables at an interactive terminal.
  See [doc/output.md](doc/output.md).
- One-time login per provider, tokens validated before storing; environment
  variables always take precedence. See [doc/auth.md](doc/auth.md).
- Self-installing agent skills, one per provider, for Claude Code and every
  agent reading `~/.agents/skills/`. See
  [doc/agent-skills.md](doc/agent-skills.md).
- Self-update from GitHub Releases with `foac update`.

## Providers

| Provider | Covers | Docs |
| --- | --- | --- |
| Linear | Issues, projects, teams, users, cycles, labels, workflow states, documents, initiatives, milestones, status updates, attachments | [doc/linear.md](doc/linear.md) |
| GitHub | Repositories, issues, pull requests, reviews, Actions, branches, commits, checks, releases, labels, artifacts, collaborators | [doc/github.md](doc/github.md) |
| Sentry | Organizations, projects, issues, error events, releases | [doc/sentry.md](doc/sentry.md) |
| Slack | Conversations, messages, threads, users, message search, reactions | [doc/slack.md](doc/slack.md) |

More providers are on the way; candidates are tracked in
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
foac --help
foac auth linear login
foac linear issue list --team ENG --state "In Progress"
foac update
```

`foac update` downloads the latest GitHub release for this platform and replaces the running binary.

Other commands check GitHub for a newer release at most once a day, and print a notice on stderr while one exists. They never auto-install. Set `FOAC_NO_UPDATE_CHECK` (or `CI`) to skip the check.
