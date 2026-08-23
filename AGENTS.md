# Hacking on foac

```sh
cargo run -- --help
cargo test --locked
```

The layout is two files: `src/main.rs` parses the top-level CLI and
dispatches, `src/linear.rs` holds every Linear command. Queries live in
`graphql/linear/queries.graphql`, and `graphql_client` generates their Rust
types at compile time against the vendored `graphql/linear/schema.graphql`
(51k lines — grep it, don't read it whole). It can be refreshed from
<https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql>.

To add a Linear command:

1. Write the query or mutation in `graphql/linear/queries.graphql`.
2. Register its name in the `linear_query!` list in `src/linear.rs`.
3. Add the clap subcommand variant and its dispatch arm, following any
   existing resource (e.g. `LabelCmd`).
4. Update `doc/SKILL.md` in the same change if the CLI surface or conventions
   changed — it is compiled into the binary (`foac skill`) and installed into
   agents' skill folders, so it must always match the CLI.

Every command follows the same conventions: raw API JSON on stdout, errors on
stderr with exit code 1, nothing interactive. `list` filter flags accept a
UUID or a human name (see `eq_filter`); `create`/`update` flags require UUIDs.

Releases are automated: a push to master touching `src/`, `graphql/`,
`doc/SKILL.md`, or the Cargo files bumps the version from conventional-commit
prefixes (`feat!:` major, `feat:` minor, anything else patch), then builds and
publishes the binaries. There is no manual release step. Use
conventional-commit prefixes accordingly.
