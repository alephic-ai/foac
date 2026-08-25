# Releasing

Releases are automated: a push to main touching `src/`, `assets/`, or
the Cargo files bumps the version from conventional-commit
prefixes (`feat!:` major, `feat:` minor, anything else patch), then builds and
publishes the binaries. There is no manual release step. Use
conventional-commit prefixes accordingly.
