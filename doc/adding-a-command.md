# Adding a command

New resources or verbs on an existing provider. A whole new provider follows
[adding-a-provider.md](adding-a-provider.md).

1. For Linear only: write the query or mutation in
   `assets/graphql/linear/queries.graphql` and register its name in the
   `linear_query!` list in `src/linear.rs`.
2. Add the clap subcommand variant and its dispatch arm in the provider's
   file, following any existing resource there. REST providers build the
   request with their `path!` macro and the
   `rest::push_query`/`rest::insert_opt` helpers. Dispatch arms return the
   response JSON — through `api.print` (or the provider-local `print_list`
   for lists) on REST, or back to `run` for Linear — never printing success
   output themselves. A `get` verb with an identifying positional argument
   makes it `Option<_>`, flattens `pipe::FromFlag`, and dispatches through
   `pipe::run_get` with a fetch-one closure, so piped `list` output joins
   into it.
3. Update `assets/SKILL.md` in the same change if the CLI surface or conventions
   changed. It is compiled into the binary and installed into agents' skill
   folders, so it must always match the CLI.
