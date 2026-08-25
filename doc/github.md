# GitHub

`foac github` talks to GitHub.com's REST API. It uses `GITHUB_TOKEN`, a credential
saved by `foac auth github login`, or `gh auth token`, in that order.
Repository-scoped commands accept `--repo OWNER/NAME`; without it, foac uses the
current checkout's GitHub remote.

For classic tokens, the `repo` scope covers private-repository commands. For
fine-grained tokens, grant Metadata read plus read or write access (matching the
commands you will use) to Issues, Pull requests, Actions, Checks, Commit
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

## Entity relationships

Entities exposed by the CLI and how they relate. Everything except
`repo list` is rooted at one repository. Issues and pull requests share a
single comment store, so `issue comment` commands work on both.

```mermaid
erDiagram
    REPO ||--o{ ISSUE : has
    REPO ||--o{ PULL : has
    REPO ||--o{ BRANCH : has
    REPO ||--o{ REF : has
    REPO ||--o{ COMMIT : has
    REPO ||--o{ LABEL : defines
    REPO ||--o{ RELEASE : publishes
    REPO ||--o{ WORKFLOW : defines
    REPO ||--o{ ARTIFACT : stores
    REPO }o--o{ COLLABORATOR : "shared with"
    ISSUE ||--o{ COMMENT : has
    PULL ||--o{ COMMENT : has
    ISSUE }o--o{ LABEL : "tagged with"
    PULL }o--o{ LABEL : "tagged with"
    PULL ||--o{ REVIEW : receives
    BRANCH ||--o| BRANCH_PROTECTION : "guarded by"
    COMMIT ||--o{ COMMIT_COMMENT : has
    COMMIT ||--o{ STATUS : reports
    COMMIT ||--o{ CHECK_SUITE : runs
    CHECK_SUITE ||--o{ CHECK_RUN : contains
    WORKFLOW ||--o{ RUN : triggers
    RUN ||--o{ JOB : contains
    RUN ||--o{ ARTIFACT : produces
    RELEASE ||--o{ RELEASE_ASSET : ships
```
