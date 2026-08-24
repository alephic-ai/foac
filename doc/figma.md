# Figma

`foac figma` talks to [Figma's REST API](https://developers.figma.com/docs/rest-api/).
It uses `FIGMA_ACCESS_TOKEN` or a personal access token saved by
`foac auth figma login`. When generating the token, grant
`current_user:read` (login validates against `/v1/me`, so this one is
required), then the scopes for the commands you use: `folders:read` for
`project list` and `file list`, `file_content:read` for `file get`,
`file nodes`, and `image export`, `file_versions:read` for `file versions`,
and `file_comments:read`/`file_comments:write` for comments.
File arguments accept a raw file key or a pasted
figma.com file URL; team and project IDs are the numbers in figma.com URLs
(they are not discoverable through the API). Node IDs use colons like `1:2`,
and the `node-id=1-2` form from URLs is converted automatically.

```sh
export FIGMA_ACCESS_TOKEN=figd_...
foac figma file get 'https://www.figma.com/design/AbC123/My-File' --depth 2
foac figma file nodes AbC123 --ids 1:2,3:4
foac figma comment create AbC123 --body "Implemented, see PR #42"
foac figma image export AbC123 --ids 1:2 --image-format png --scale 2
foac figma --help
```

`file get` returns the whole document tree, which can be very large; scope it
with `--depth` or `--ids`. Only `file versions` paginates: pass
`pageInfo.nextBefore` as `--before` while `hasNextPage` is true. Comments can
be listed, created (with `--reply-to` for replies), and deleted, but not
resolved: the API keeps `resolved_at` read-only. `image export` returns
node-to-URL mappings, not image data, and the URLs expire after 30 days.

`project list` and `file list` use Figma's v1 endpoints, which Figma has
deprecated in favor of v2 folder endpoints; they still work with personal
access tokens, and moving to v2 is a follow-up.
