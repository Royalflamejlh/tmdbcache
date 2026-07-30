# Security Policy

## Supported versions

This is a hobby project with a single active line of development. Fixes land on
`main` and in the next image build; there are no backported releases.

## Reporting a vulnerability

Please report privately rather than in a public issue:

- Use GitHub's [private vulnerability reporting](https://github.com/Royalflamejlh/tmdbcache/security/advisories/new), or
- email <git@johnlhoward.me>.

Include what you need to demonstrate the issue — a request, a config, a log excerpt.
I'll acknowledge within a week or so; this is not a staffed project, so please don't
expect a same-day response.

## Threat model, honestly stated

**This application has no authentication.** The original gated access behind Keycloak;
that is not implemented here, and `/api/v1/tmdb/configuration` reports
`requireLogin: false` and `oauth2Enabled: false` so no client mistakenly believes
otherwise.

Anyone who can reach the port can read the library, add and delete titles, and cause
outbound TMDB requests using your API key. **Do not expose it directly to the
internet.** Put it behind a reverse proxy that handles authentication, or keep it on
a trusted network.

Given that, the following are in scope as vulnerabilities:

- Path traversal or arbitrary file read through `imagePath`, `backdropSize` or the
  wallpaper endpoint.
- SQL injection — note that all dynamic SQL is built from closed enums and generated
  placeholder lists, never from request input.
- Anything that lets a request escape the configured `imageCache` / `database`
  directories.
- Leaking `MOVIEDB_TMDB_APIKEY` into a response body, a log line, or an error message.
- Container escape, or the service running as root when `PUID`/`PGID` were set.

Out of scope:

- "The API needs no authentication" — known and documented above.
- Anything requiring an already-root process inside the container.
- Scripts you mounted yourself at `/custom-cont-init.d`; those run as root by design,
  which is why the container warns when that directory is not root-owned.
