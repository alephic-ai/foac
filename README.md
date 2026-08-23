# foac

foac, the Father Of All CLIs

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

## Linear

`foac linear` talks to [Linear's GraphQL API](https://linear.app/developers/graphql). It needs a personal API key in `LINEAR_API_KEY`. Every command prints JSON on stdout; list commands paginate with `--limit`/`--after` and include `pageInfo` in the output.

```sh
export LINEAR_API_KEY=lin_api_...
foac linear issue list --team ENG --state "In Progress"
foac linear issue create --team <TEAM_UUID> --title "Fix the flux capacitor"
foac linear --help
```

The vendored schema in `graphql/linear/schema.graphql` can be refreshed from
<https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql>.

## Agents

`foac skill` prints an agent skill explaining how the CLI is structured and its
conventions. `foac skill install` writes it into the skill folders of the
agents found on the machine: `~/.claude/skills/` for Claude Code, and the
cross-agent standard `~/.agents/skills/` read by Cursor, Codex, Gemini CLI,
GitHub Copilot, OpenCode, Amp, and others.

## From source

```sh
cargo run -- --help
```
