# The context tax, measured

The README claims foac replaces context spent on MCP tool catalogs and API
doc fetches. This table puts a number behind that claim. Every number here
comes from [`bin/context-tax.py`](../bin/context-tax.py); the script owns
the numbers, and the table below is a snapshot from foac 2.25.1 that will
drift:

```sh
python3 bin/context-tax.py
```

The script builds the repo and measures `target/debug/foac` (never the
`foac` on PATH, which can lag a release) under a throwaway `HOME` with only
a dummy `LINEAR_API_KEY`, so exactly one provider is visible. That is the
shape foac recommends (enable only what a machine needs) and the only shape
that reproduces across machines. The provider list comes from
`foac provider list --format json`. The MCP baseline needs Docker; without
it the foac rows still print and the script exits non-zero.

Every row is bytes per request, not a one-off cost. A system prompt is
re-sent on every model call, and so is an eagerly transmitted MCP catalog;
a task that takes twenty exchanges pays it twenty times.

| Route | Bytes per request |
| --- | --- |
| foac, always-on skill listing | 208 B |
| foac, progressive discovery | 952 → 1 027 → 726 → 1 441 B |
| foac, skill route | 8 363 B (one provider skill, on trigger) |
| MCP baseline: official GitHub MCP server v1.11.0 | 120 684 B (44 tools) |

## foac, always-on

What Claude Code injects into the system prompt so the agent knows what it
can trigger: one listing line per installed skill, built from the `name:`
and `description:` frontmatter values. The count is the summed UTF-8 byte
length of the two values; keys, colons, and newlines are excluded.
`foac skill install` writes only the skills of active providers, so in the
bench's one-provider shape this row is Linear's line alone, 208 B.

With all 10 providers enabled and authenticated the sum is 1 825 B. That is
a different shape from the rest of the table, and not the default one
either, since a fresh config ships with confluence and jira disabled:

| Skill | Bytes |
| --- | --- |
| foac-axiom | 180 |
| foac-confluence | 130 |
| foac-firecrawl | 367 |
| foac-github | 204 |
| foac-jira | 140 |
| foac-linear | 208 |
| foac-neon | 175 |
| foac-sentry | 135 |
| foac-slack | 144 |
| foac-vercel | 142 |

## foac, progressive discovery

Walking the `--help` chain down to a runnable command, with Linear the only
visible provider: `foac --help` 952 B → `foac linear --help` 1 027 B →
`foac linear issue --help` 726 B → `foac linear issue list --help` 1 441 B.
Walking the chain costs the agent four tool-call turns, not just the prefill
bytes.

## foac, skill route

An agent that triggers a skill loads one provider skill, between 8 363 B
(`foac-linear`) and 10 861 B (`foac-firecrawl`). Loading all ten would cost
94 312 B. This row is only cheap in harnesses that lazy-load skills on
trigger (Claude Code does); a harness that injects everything under
`~/.agents/skills/` pays the full skill per provider up front. That is
still under one GitHub MCP catalog, but it is a different comparison.

## MCP baseline

The `tools/list` payload of the official GitHub MCP server
(`ghcr.io/github/github-mcp-server:v1.11.0`, the same server
[arXiv:2608.08654](https://arxiv.org/abs/2608.08654) used), measured live
over stdio with a dummy token. The count is every byte the server writes
for the `tools/list` response line, trailing newline included. The wire
bytes are the number, never a re-serialisation: `json.dumps` gives
123 907 B, compact separators less.

The same caveat as the skill row applies in the other direction: this is
the cost in a harness that transmits the catalog eagerly. The paper's one
MCP-capable scaffolding that defers the catalog was its cheapest MCP arm by
2×, and Anthropic has shipped lazy MCP tool loading since; a deferring
harness pays less.

GitHub only: Linear, Atlassian, and Slack MCP servers are hosted-only,
return 401 to an unauthenticated `tools/list`, and have no official image,
so no reader could re-verify a measurement.

## Scope guard

From that same paper: it explicitly failed to find a stable MCP-vs-CLI cost
ratio (13 paired comparisons spanning 0.43× to 29×), found scaffolding
choice worth 5 to 28× on its own, and found agents frequently leaking to an
interface other than the one assigned. So this measures the catalog tax
only, prefill bytes per request, and must not be presented as an end-to-end
cost or success-rate comparison. Claiming more needs a real bench with
API-side verification and per-call tool-name logging, which is a separate
repo.

---

Bytes to tokens: roughly 4 B/token for English and JSON, enough for
order-of-magnitude comparisons. These are wire bytes, not what the model
sees after the harness re-encodes tool schemas into its API's tool format;
fine at this order of magnitude, which is all this doc claims.
