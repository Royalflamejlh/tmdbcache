# Contributing

Thanks for taking a look. This is a small project, so the process is light.

## Getting set up

```bash
git clone https://github.com/Royalflamejlh/tmdbcache
cd tmdbcache
cargo test
```

No TMDB key and no network are needed for the test suite. It runs against an
in-memory SQLite database and drives the real axum router.

To run the server you do need a key:

```bash
export MOVIEDB_TMDB_APIKEY=your_key
export MOVIEDB_KEYCLOAK_ENABLED=false
cargo run
```

## Before you open a PR

`main` is protected and CI is a required check, so make these pass locally first:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

If you touched the Dockerfile, the s6 tree under `docker/root/`, or a workflow:

```bash
docker build -t tmdbcache:dev .
docker run --rm -p 8081:8081 \
  -e MOVIEDB_TMDB_APIKEY=dummy -e MOVIEDB_KEYCLOAK_ENABLED=false \
  -e PUID=$(id -u) -e PGID=$(id -g) \
  tmdbcache:dev
```

A dummy key is fine for checking that the container boots, serves the UI and answers
`/health`; only the TMDB-backed endpoints will fail.

Workflows are linted with [actionlint](https://github.com/rhysd/actionlint):

```bash
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:latest
```

## House style

- **Match the surrounding code.** Comment density, naming and idiom are fairly
  consistent; please keep it that way.
- **Comments explain *why*, not *what*.** The existing ones flag non-obvious
  constraints: why overrides live in separate columns, why bind parameters are
  chunked, why unrecognised API paths answer with JSON. Aim for that.
- **Keep SQL inside `store::sqlite`.** Everything above it goes through the `Store`
  trait. If you need a new query, add a trait method.
- **Dynamic SQL needs justification.** sqlx 0.9 requires `AssertSqlSafe` for
  non-literal SQL; only use it where the interpolated part comes from a closed enum
  or a generated placeholder list, never from request input.
- **Preserve the wire format.** Field names come from
  [`docs/openapi-original.yaml`](docs/openapi-original.yaml), including the
  inconsistent casing. Changing one is a breaking change for existing clients, so
  call it out explicitly if you mean to.

## Tests

New behaviour should come with a test. The two suites are:

- `tests/store.rs`: persistence semantics against an in-memory database.
- `tests/api.rs`: the full router, via `tower::ServiceExt::oneshot`.

Prefer a test that would have caught the bug. Assertions carry a message where the
failure would otherwise be cryptic, for example `"user override should win over the
upstream poster"`.

## Commits and PRs

- Conventional-ish prefixes are used by Dependabot (`deps`, `ci`, `docker`); for your
  own commits, a clear imperative subject is enough.
- One logical change per PR where you can manage it.
- Describe what you verified, not just what you changed. If something is untested,
  say so. That's more useful than a confident-sounding summary.

## Reporting bugs

Include:

- what you ran (compose file or `docker run` line, with the key redacted),
- the container's startup banner (it prints the version, uid/gid and umask),
- the relevant log lines,
- whether it reproduces with a fresh `database/` directory.

## Security

Please don't open a public issue for a vulnerability. See
[SECURITY.md](SECURITY.md).
