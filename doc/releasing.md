# Releasing

Releases are automated: a push to main touching `src/`, `assets/`, or
the Cargo files bumps the version from conventional-commit
prefixes (`feat!:` major, `feat:` minor, anything else patch), then builds and
publishes the binaries. There is no manual release step. Use
conventional-commit prefixes accordingly.

Once the GitHub release is public, the crate goes to
[crates.io](https://crates.io/crates/foac) with `cargo publish`. It
authenticates by Trusted Publishing — crates.io is configured to trust this
repository's `release.yml`, and hands the job a short-lived token — so there is
no registry secret in the repo.

Two more jobs repackage the same release binaries for the two registries
agent harnesses already have on hand:

- **PyPI**, for `uvx foac`. `ci/build_wheels.py` wraps each binary in a wheel —
  a zip whose `.data/scripts/foac` lands on PATH, so the package holds no
  Python at all. Linux ships the static musl build under both the manylinux and
  the musllinux tag, since a binary linking no libc honours either promise.
  Trusted Publishing again, and PyPI accepts a pending publisher for a project
  that does not exist yet, so the first release needs no setup.
- **npm**, for `npx @alephic/foac`. Six `@alephic/foac-<os>-<cpu>` packages,
  one binary each, plus an `@alephic/foac` package whose
  `optionalDependencies` list all six and whose `bin` is `npm/foac.js`, the
  shim that runs whichever one npm's `os`/`cpu` filtering installed. The
  wrapper is scoped because npm rejects the bare name: its similarity filter
  reads `foac` as too close to `cac`, `flat`, `solc`, `koa` and `soap`, and
  scoped names skip that check. This is also the one registry with a secret —
  npm cannot configure a trusted publisher for a package that does not exist
  yet, so publishing uses `NPM_TOKEN`, a granular token with write access to
  the `@alephic` scope. Migrating a package to OIDC later means configuring it
  on npmjs.com and dropping the secret.

Both jobs check before they publish — the wheel into a venv, the shim against a
linked platform package — because a broken installer is only visible on install.

A last job regenerates `Formula/foac.rb` in
[alephic-ai/homebrew-tap](https://github.com/alephic-ai/homebrew-tap), the tap
shared by every open-source Alephic tool, from this release's tarballs and
their `.sha256` sidecars, so `brew install alephic-ai/tap/foac` tracks every
release. The formula is generated, never hand-edited: fix a bad one in
`release.yml`, not in the tap.
