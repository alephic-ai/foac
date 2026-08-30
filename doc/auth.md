# Authentication

Log in once per provider; every agent harness and script on the machine
reuses the same stored credential. Provider toggles are independent of auth:
`foac provider disable` hides a provider without touching its credentials, so
re-enabling never means re-authenticating.

foac can validate every provider at once or manage each provider separately:

```sh
foac auth status
foac auth linear status
foac auth linear login
foac auth linear logout
foac auth github status
foac auth github login
foac auth github logout
foac auth neon status
foac auth neon login
foac auth neon logout
foac auth jira status
foac auth jira login
foac auth jira logout
foac auth confluence status
foac auth confluence login
foac auth confluence logout
foac auth sentry status
foac auth sentry login
foac auth sentry logout
foac auth slack status
foac auth slack login
foac auth slack logout
foac auth vercel status
foac auth vercel login
foac auth vercel logout
foac auth firecrawl status
foac auth firecrawl login
foac auth firecrawl logout
```

`login` prints a link and permission guidance, securely prompts for a personal
API token, validates it, and stores it in foac's machine-managed credentials
file (`~/.config/foac/credentials.json`, or under `XDG_CONFIG_HOME`), which
foac atomically replaces and keeps mode `0600` on Unix before writing secret
bytes. Editable provider settings live separately in
`~/.config/foac/config.toml`. Pipe a token to `login` for non-interactive
use.

## Instances

Every provider can hold several *instances*: independent, named logins to
different tenants of the same product (two Slack workspaces, two Atlassian
sites, a SaaS and a self-hosted Sentry or Firecrawl). An unnamed `login`
creates the instance named `default`, which is what unqualified commands
use — a single-tenant setup never has to think about instances.

```sh
foac auth slack login --instance workb   # log the "workb" workspace in
foac slack conversation list -i workb    # use it explicitly
foac auth slack logout -i workb          # remove only that instance
foac auth status                         # lists every stored instance, keyed slack@workb
```

Instance names use lowercase letters, digits, `-`, and `_`. The credentials
file nests them per provider (`{"slack": {"default": {...}, "workb": {...}}}`);
Jira and Confluence share the vendor-level `atlassian` entry, so an Atlassian
instance covers both. A named Sentry or Firecrawl instance stores its base
URL alongside its token (`foac auth <provider> login --host ...`).

A provider command picks its instance in this order:

1. the global `-i`/`--instance` flag
2. the nearest `.foac.toml` `[defaults]` table, then the global
   `config.toml` one:

   ```toml
   [defaults]
   slack = "workb"   # unqualified slack commands here use workb
   ```

3. the `default` instance

**Environment tokens belong to the default instance only.** When a named
instance is selected, `SLACK_BOT_TOKEN`, `GITHUB_TOKEN`, the `gh` CLI
fallback, `SENTRY_URL`, `FIRECRAWL_API_URL`, and the rest never apply: a
named instance reads exactly its stored credentials, so an ambient token from
workspace A can never leak into commands aimed at workspace B. Auth commands
use the flag only — never folder defaults — so a `login` is never silently
redirected.

Instances can be enabled or disabled like providers, globally or per folder:
`foac provider disable slack --instance workb [--local]` writes the qualified
name `slack@workb` into the usual toggle arrays, and a command that resolves
to a disabled instance refuses to run. A bare provider toggle still governs
the provider as a whole, all instances included.
Slack login prompts for both bot and user tokens; for non-interactive use, pipe
two lines in that order (either line may be blank). It also links to Slack's app
management page and prints a ready-to-paste JSON app manifest with foac's
recommended bot and user scopes. Tokens are never printed.

Jira and Confluence need three values: the Atlassian site host (like
`acme.atlassian.net`), the account email, and an API token. Login prompts for
all three, validates them against the calling provider's API, and stores them
as one vendor-level `atlassian` credential shared by both providers: logging
in through either covers both, and logging out of either removes the
credential for both. For non-interactive use, pipe one line per missing value
in host, email, token order; `--host` and `--email` skip their line.
`ATLASSIAN_HOST`, `ATLASSIAN_EMAIL`, and `ATLASSIAN_API_TOKEN` each override
their stored counterpart independently.

For the default instance, environment variables take precedence over stored
credentials, and GitHub also falls back to `gh auth token` when neither
`GITHUB_TOKEN` nor a stored foac credential is available. `logout` removes
only foac's stored credentials for the selected instance (both bot and user
credentials for Slack); it does not unset environment variables, log out the
`gh` CLI, or revoke tokens at the provider.

Status commands validate credentials with the provider and print JSON. The
all-provider command prints an object keyed by provider, plus one
`provider@instance` entry per stored named instance; provider-specific
commands print one status object. A provider's `status` is `authenticated`,
`unauthenticated`, or `error`, and includes the credential `source` and safe
account identity when available. Status commands exit zero after printing the
report, so callers should inspect the JSON status values.
