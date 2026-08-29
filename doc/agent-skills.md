# Agent skills

`foac skill print <provider>` prints that provider's agent skill explaining
how the CLI is structured and its conventions. `foac skill install` writes one
skill per active provider (`foac-github`, `foac-linear`, ...) into the skill
folders of the agents found on the machine (`~/.claude/skills/` for Claude
Code, and the cross-agent standard `~/.agents/skills/` read by Cursor, Codex,
Gemini CLI, GitHub Copilot, OpenCode, Amp, and others) and removes the skills
of providers that are disabled or unauthenticated. Existing skills are only
written when their contents changed; byte-identical skills are reported as
`Unchanged`.

`foac update` refreshes the foac provider skills already installed in either
location. It preserves the installed provider set instead of adding or removing
skills, and reports byte-identical skills as `Unchanged` without rewriting them.

Upgrades that bypass `foac update` refresh them too. `brew upgrade foac` cannot
run `foac skill install` itself — Homebrew's post-install hook is sandboxed with
`deny_read_home`, so it can reach neither the config nor the skill folders — and
`ubi`, `cargo install`, `uv` and `npm` have no hook at all. So the new binary
does it: the
first command after a version change re-renders the installed skills, then
stamps the version in `~/.cache/foac/skills-version` (or `XDG_CACHE_HOME`).
Every later run is one small read of that stamp. The refresh preserves the
installed provider set, like `foac update`, and only names on stderr the skills
it actually rewrote.
