# Hacking on foac

```sh
cargo run -- --help
cargo test --locked
```

- [ARCHITECTURE.md](ARCHITECTURE.md): the theory (what foac is, key
  decisions and their rules, where it grows). Read it first.
- [doc/layout.md](doc/layout.md): the mechanics (lib/bin split, GraphQL
  codegen, printer and provider details). Read it before touching `src/`.
- [doc/adding-a-command.md](doc/adding-a-command.md): the recipe for a new
  command on an existing provider.
- [doc/adding-a-provider.md](doc/adding-a-provider.md): the recipe for a
  whole new provider.
- [doc/releasing.md](doc/releasing.md): the automated release flow. Commit
  messages need conventional-commit prefixes because version bumps derive
  from them.

Always: keep `doc/SKILL.md` in sync with any CLI surface change, and grep
`graphql/linear/schema.graphql` (51k lines) rather than reading it whole.
