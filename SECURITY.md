# Security Policy

## Supported versions

This is a hobby project with a single line of development. Fixes land on `main` and in
the next image build. There are no backported releases.

## Reporting a vulnerability

Please report privately rather than in a public issue:

- Use GitHub's [private vulnerability reporting](https://github.com/Royalflamejlh/tmdbcache/security/advisories/new), or
- email <git@johnlhoward.me>.

Include whatever you need to demonstrate the issue: a request, a config, a log
excerpt. I'll acknowledge within a week or so. This isn't a staffed project, so please
don't expect a same-day reply.

## Authentication

There is no authentication at the moment. Anyone who can reach the port can read the
library, add and delete titles, and spend your TMDB quota.

Until that changes, run it behind a reverse proxy that handles authentication, or keep
it on a trusted network. `/api/v1/tmdb/configuration` reports `requireLogin: false`
and `oauth2Enabled: false` so a client is never misled about it.

Adding authentication is a welcome contribution. Open an issue or a PR for it rather
than a security report, since the current state is already known and documented here.

## Scope

In scope:

- Path traversal or arbitrary file read through `imagePath`, `backdropSize` or the
  wallpaper endpoint.
- SQL injection. All dynamic SQL is built from closed enums and generated placeholder
  lists, never from request input, so a way around that is worth reporting.
- Anything that lets a request escape the configured `imageCache` or `database`
  directories.
- Leaking `MOVIEDB_TMDB_APIKEY` into a response body, a log line or an error message.
- Container escape, or the service running as root when `PUID` and `PGID` were set.

Out of scope:

- Anything that requires an already-root process inside the container.
- Scripts you mounted yourself at `/custom-cont-init.d`. Those run as root by design,
  which is why the container warns when that directory isn't root-owned.
