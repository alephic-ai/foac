# Adding a REST command

1. Add the clap subcommand variant and its dispatch arm in the provider's
   file, following any existing resource there. Build the request with the
   provider's `path!` macro and the `rest::push_query`/`rest::insert_opt`
   helpers; dispatch arms return the response JSON through `api.print` (or
   the provider-local `print_list` for lists), never printing success output
   themselves.
2. Update `doc/SKILL.md` in the same change if the CLI surface or conventions
   changed. It is compiled into the binary and installed into agents' skill
   folders, so it must always match the CLI.

## Adding a REST provider

Follow `src/sentry.rs`'s shape on top of `src/rest.rs`: a `Cmd` clap tree, a
`run` that builds a `rest::Api` (bearer or Basic auth, static provider
headers, optional trailing slash), `authenticated()`, an
`auth_identity` built on `rest::identity`, and a provider-local `print_list`
that wraps arrays with `rest::wrap_list` and the provider's own `pageInfo`
parsing. Every provider addition also touches: `src/lib.rs`, `auth.rs`
(Provider enum, token resolution, identity), `provider.rs`'s `PROVIDERS` and
`Credential`, `main.rs`'s `providers()` and dispatch, `doc/SKILL.md` marker
blocks, `.github/workflows/skill.yml`, `README.md`, `doc/<provider>.md`, and
`doc/auth.md`. Test HTTP against `rest::testing::test_server`, never the real
API.
