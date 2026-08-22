---
name: foac
description: Use the foac CLI to interact with external services from the shell, currently Linear (issues, comments, projects, teams, users, cycles, labels, workflow states, documents, initiatives, milestones, status updates, attachments). Use when asked to read, create, or update anything in Linear.
---

# foac

foac wraps external service APIs as CLI subcommands, designed for LLM agents:
every command prints compact JSON on stdout, errors go to stderr with exit
code 1, and nothing is interactive.

## Structure

```
foac <provider> <resource> <verb> [flags]
```

- Providers: `linear` (more will be added).
- Resources are nouns (`issue`, `project`, `team`, `user`, ...), verbs are
  `list`, `get`, `create`, `update`, `delete`.
- `--help` at any level is the ground truth for what exists and which flags
  each verb takes. Explore with `foac linear --help`, then
  `foac linear <resource> --help`.

## Conventions

- **Auth**: Linear needs a personal API key in the `LINEAR_API_KEY`
  environment variable.
- **Output**: the raw API response as JSON on stdout. Failures print the
  API's error JSON on stderr and exit non-zero.
- **Pagination**: `list` verbs take `--limit N` (default 50) and
  `--after CURSOR`, and their output includes
  `pageInfo {hasNextPage, endCursor}`. To get everything, loop passing
  `endCursor` to `--after` until `hasNextPage` is false.
- **Filters accept names**: filter flags on `list` verbs take a UUID or a
  human value — a team key (`ENG`), a state name (`In Progress`), a user
  email or display name, a project or label name.
- **Mutations need UUIDs**: flags on `create`/`update` verbs (`--assignee`,
  `--state`, `--project`, ...) require UUIDs. Look them up first with the
  matching `list` command. Issues are the exception: `get`, `update`, and
  `--issue` flags accept an identifier like `ENG-123` as well.
- **Updates are partial**: `update` verbs only change the flags you pass;
  omitted flags keep their value. Fields cannot be cleared to null.

## Examples

```sh
foac linear issue list --team ENG --state "In Progress"
foac linear issue create --team <TEAM_UUID> --title "Fix login" --description "..."
foac linear comment create --issue ENG-123 --body "Done, see PR #42"
```

## Maintenance

`foac update` replaces the binary with the latest release; `foac version`
prints the installed version. Regenerate this skill with `foac skill` after
updating.
