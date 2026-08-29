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

A last job regenerates `Formula/foac.rb` in
[alephic-ai/homebrew-tap](https://github.com/alephic-ai/homebrew-tap), the tap
shared by every open-source Alephic tool, from this release's tarballs and
their `.sha256` sidecars, so `brew install alephic-ai/tap/foac` tracks every
release. The formula is generated, never hand-edited: fix a bad one in
`release.yml`, not in the tap.
