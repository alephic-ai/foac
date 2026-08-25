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

A whole new provider (not just a command) follows
[adding-a-provider.md](adding-a-provider.md).
