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
foac auth sentry status
foac auth sentry login
foac auth sentry logout
foac auth slack status
foac auth slack login
foac auth slack logout
```

`login` prints a link and permission guidance, securely prompts for a personal
API token, validates it, and stores it in foac's config file
(`~/.config/foac/config.json`, or under `XDG_CONFIG_HOME`), which foac keeps
readable by the owner only. Pipe a token to `login` for non-interactive use.
Slack login prompts for both bot and user tokens; for non-interactive use, pipe
two lines in that order (either line may be blank). It also links to Slack's app
management page and prints a ready-to-paste JSON app manifest with foac's
recommended bot and user scopes. Tokens are never printed.

Environment variables take precedence over stored credentials. GitHub also
falls back to `gh auth token` when neither `GITHUB_TOKEN` nor a stored foac
credential is available. `logout` removes only foac's stored credentials
(both bot and user credentials for Slack); it does not unset environment
variables, log out the `gh` CLI, or revoke tokens at the provider.

Status commands validate credentials with the provider and print JSON. The
all-provider command prints an object keyed by provider; provider-specific
commands print one status object. A provider's `status` is `authenticated`,
`unauthenticated`, or `error`, and includes the credential `source` and safe
account identity when available. Status commands exit zero after printing the
report, so callers should inspect the JSON status values.
