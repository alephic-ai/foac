# Adding a Linear command

1. Write the query or mutation in `graphql/linear/queries.graphql`.
2. Register its name in the `linear_query!` list in `src/linear.rs`.
3. Add the clap subcommand variant and its dispatch arm, following any
   existing resource (e.g. `LabelCmd`). Dispatch arms return the response
   JSON; `run` prints it once through the shared printer in `src/output.rs`
   (JSON or table per `--format`), so commands never print success output
   themselves.
4. Update `doc/SKILL.md` in the same change if the CLI surface or conventions
   changed — it is compiled into the binary (`foac skill print`) and installed into
   agents' skill folders, so it must always match the CLI.
