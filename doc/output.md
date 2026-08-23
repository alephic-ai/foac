# Output

Provider commands print the provider's response as compact JSON on
stdout. At an interactive terminal, foac renders it as a table sized to the
terminal width instead. Pick a format explicitly with `--format json|table|auto`
or the `FOAC_FORMAT` environment variable; pipes and CI (`CI` set) always get
JSON. Errors stay on stderr as JSON with exit code 1, and `auth`, `provider`,
`version`, `update`, and `skill` ignore `--format`.
