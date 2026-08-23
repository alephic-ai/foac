# Hacking on foac

```sh
cargo run -- --help
cargo test --locked
```

- [doc/layout.md](doc/layout.md) — code layout, GraphQL codegen, CLI
  conventions. Read it before touching `src/`.
- [doc/adding-a-command.md](doc/adding-a-command.md) — the recipe for a new
  Linear command.
- [doc/releasing.md](doc/releasing.md) — the automated release flow; commit
  messages need conventional-commit prefixes because version bumps derive
  from them.

Always: keep `doc/SKILL.md` in sync with any CLI surface change, and grep
`graphql/linear/schema.graphql` (51k lines) rather than reading it whole.
