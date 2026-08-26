# Output

Provider commands print the provider's response as compact JSON on
stdout. At an interactive terminal, foac renders it as a table sized to the
terminal width instead. List columns keep the API's field order; when a row
has more fields than the terminal fits (REST rows carry ~80), only the
leading columns render and a note counts the rest. Pick a format explicitly with `--format json|table|auto`
or the `FOAC_FORMAT` environment variable; pipes and CI (`CI` set) always get
JSON. Errors stay on stderr as JSON with exit code 1, and `auth`, `provider`,
`about`, `version`, `update`, and `skill` ignore `--format`.
