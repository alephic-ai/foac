# Sentry

`foac sentry` talks to [Sentry's REST API](https://docs.sentry.io/api/). It
uses `SENTRY_AUTH_TOKEN` or a credential saved by `foac auth sentry login`.
Pass `--org SLUG` or set `SENTRY_ORG`. At an interactive terminal,
`foac auth sentry login` first asks for the Sentry hostname (default
`sentry.io`, always https; enter your own for a self-hosted instance) and
saves it alongside the token; piped logins read only the token, so pass
`--host sentry.example.com` to save a self-hosted host non-interactively.
The host is stored with the instance's credentials, so a SaaS and a
self-hosted Sentry can coexist as named instances (see
[auth.md](auth.md)). `SENTRY_URL` overrides the saved host for the default
instance only. Issue commands accept numeric IDs or short IDs like
`PROJ-123`.

```sh
export SENTRY_AUTH_TOKEN=sntrys_...
foac sentry issue list --org acme --project backend --query "is:unresolved"
foac sentry issue latest-event PROJ-123 --org acme
foac sentry --help
```

Sentry list commands print `{"items":[...],"pageInfo":{...}}` and paginate
with `--cursor` using `pageInfo.nextCursor`. Releases are read-only; release
creation and sourcemap upload stay with `sentry-cli`.

## Entity relationships

Entities exposed by the CLI and how they relate. An assignee is a user or a
`#team-slug`; issues relate to releases only through search queries like
`release:1.2.0`, not a direct edge.

```mermaid
erDiagram
    ORG ||--o{ PROJECT : owns
    PROJECT ||--o{ ISSUE : groups
    ISSUE ||--o{ EVENT : aggregates
    ORG ||--o{ RELEASE : tracks
    ISSUE }o--o| ASSIGNEE : "assigned to"
    classDef scope fill:#bbdefb,stroke:#1565c0,color:#000
    classDef work fill:#c8e6c9,stroke:#2e7d32,color:#000
    classDef people fill:#ffe0b2,stroke:#e65100,color:#000
    classDef release fill:#fff9c4,stroke:#f57f17,color:#000
    class ORG,PROJECT scope
    class ISSUE,EVENT work
    class ASSIGNEE people
    class RELEASE release
```
