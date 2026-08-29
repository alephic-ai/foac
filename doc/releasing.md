# Releasing

Releases are automated: a push to main touching `src/`, `assets/`, or
the Cargo files bumps the version from conventional-commit
prefixes (`feat!:` major, `feat:` minor, anything else patch), then builds and
publishes the binaries. There is no manual release step. Use
conventional-commit prefixes accordingly.

A last job regenerates `Formula/foac.rb` in
[alephic-ai/homebrew-tap](https://github.com/alephic-ai/homebrew-tap), the tap
shared by every open-source Alephic tool, from this release's tarballs and
their `.sha256` sidecars, so `brew install alephic-ai/tap/foac` tracks every
release. The formula is generated, never hand-edited: fix a bad one in
`release.yml`, not in the tap.
