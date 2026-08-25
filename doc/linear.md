# Linear

`foac linear` talks to [Linear's GraphQL API](https://linear.app/developers/graphql). It uses `LINEAR_API_KEY` or a credential saved by `foac auth linear login`. Every command prints JSON on stdout; list commands paginate with `--limit`/`--after` and include `pageInfo` in the output.

```sh
export LINEAR_API_KEY=lin_api_...
foac linear issue list --team ENG --state "In Progress"
foac linear issue create --team <TEAM_UUID> --title "Fix the flux capacitor"
foac linear --help
```

## Entity relationships

Entities exposed by the CLI and how they relate. A status update and a
document each belong to a project or an initiative, never both; the CLI
enforces the exclusivity. The project↔initiative edge is not exposed.

```mermaid
erDiagram
    %% LR stacks siblings vertically: tall and narrow instead of very wide
    direction LR
    WORKSPACE ||--o{ TEAM : contains
    TEAM }o--o{ USER : "has members"
    TEAM ||--o{ ISSUE : has
    TEAM ||--o{ STATUS : defines
    TEAM ||--o{ LABEL : owns
    TEAM ||--o{ CYCLE : runs
    TEAM }o--o{ PROJECT : "works on"
    LABEL |o--o{ LABEL : groups
    ISSUE }o--|| STATUS : "is in"
    ISSUE }o--o| USER : "assigned to"
    ISSUE }o--o| PROJECT : "belongs to"
    ISSUE }o--o| CYCLE : "scheduled in"
    ISSUE }o--o{ LABEL : "tagged with"
    ISSUE |o--o{ ISSUE : "parent of"
    ISSUE ||--o{ COMMENT : has
    ISSUE ||--o{ ATTACHMENT : has
    COMMENT }o--|| USER : "written by"
    COMMENT |o--o{ COMMENT : "thread parent of"
    PROJECT ||--o{ MILESTONE : has
    PROJECT }o--o| USER : "led by"
    PROJECT }o--o{ PROJECT_LABEL : "tagged with"
    PROJECT ||--o{ STATUS_UPDATE : has
    INITIATIVE ||--o{ STATUS_UPDATE : has
    STATUS_UPDATE }o--|| USER : "written by"
    INITIATIVE }o--o| USER : "owned by"
    PROJECT ||--o{ DOCUMENT : holds
    INITIATIVE ||--o{ DOCUMENT : holds
    DOCUMENT }o--|| USER : "created by"
```
