# Agent skills

`foac skill print <provider>` prints that provider's agent skill explaining
how the CLI is structured and its conventions. `foac skill install` writes one
skill per active provider (`foac-github`, `foac-linear`, ...) into the skill
folders of the agents found on the machine — `~/.claude/skills/` for Claude
Code, and the cross-agent standard `~/.agents/skills/` read by Cursor, Codex,
Gemini CLI, GitHub Copilot, OpenCode, Amp, and others — and removes the skills
of providers that are disabled or unauthenticated.
