<div align="center">

# tmdbcache

**A self-hosted web app that catalogues your video library, backed by TMDB — and caches everything locally.**

Metadata in SQLite, artwork on disk. Once a title is cached, browsing it costs zero upstream calls.

[![CI](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/ci.yml/badge.svg)](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/ci.yml)
[![Docker](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/docker.yml/badge.svg)](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/docker.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/rust-2024%20edition-orange?logo=rust)
![Platforms](https://img.shields.io/badge/platforms-amd64%20%7C%20arm64-informational)

</div>

---

This is a Rust reimplementation of **MovieDB** — a Spring Boot + PWA app published as
[`moviedb/moviedb`](https://hub.docker.com/r/moviedb/moviedb) whose source
(`justsomebody42/movieDB`) **disappeared from GitHub**.

The image was still on Docker Hub, and it shipped its own OpenAPI document. That
document — 26 endpoints, ~40 schemas — was recovered and is reproduced verbatim in
[`docs/openapi-original.yaml`](docs/openapi-original.yaml). So this isn't a guess at
the original API; it's the real contract, reimplemented field for field. See
[`docs/PROVENANCE.md`](docs/PROVENANCE.md) for exactly how it was extracted.

## Quick start

```yaml
---
services:
  tmdbcache:
    image: ghcr.io/royalflamejlh/tmdbcache:latest
    container_name: tmdbcache
    environment:
      - MOVIEDB_TMDB_APIKEY=your_tmdb_key   # required
      - MOVIEDB_KEYCLOAK_ENABLED=false      # required; auth is not implemented
      - PUID=1000
      - PGID=1000
      - TZ=Etc/UTC
      - MOVIEDB_TMDB_REGION=US
    volumes:
      - ./database:/database
      - ./imageCache:/imageCache
    ports:
      - 8081:8081
    restart: unless-stopped
```

```bash
docker compose up -d
```

Then open <http://localhost:8081>, search for something, and click it — opening a
search result is what adds it to your library.

<details>
<summary>Without Docker</summary>

```bash
export MOVIEDB_TMDB_APIKEY=your_key_here
export MOVIEDB_KEYCLOAK_ENABLED=false
cargo run --release
```

</details>

A TMDB **v3 API key** or a **v4 read access token** both work — the key's shape is
detected and it is sent as `api_key` or as a bearer token accordingly.

## What it does

| | |
| --- | --- |
| 🔎 **Search and add** | Find movies and TV shows on TMDB; opening one caches it locally. |
| 💾 **Caches everything** | Metadata in SQLite, artwork on disk under `imageCache/<size>/`. A cached title renders with no TMDB calls at all. |
| ✅ **Tracks your state** | Watched, favorite, on-watchlist and freeform tags — per movie, show, season *and* episode. |
| ✏️ **Manual overrides** | Replace a poster, backdrop, overview or IMDb id. Overrides live in separate columns, so `?refresh=true` never clobbers an edit. |
| 🎲 **Discover** | Pools the TMDB recommendations of everything you own and surfaces the most-recommended titles you don't. |
| 🖼️ **Wallpapers** | Drop `.jpg`/`.png` files into `imageCache/wallpapers`; they appear behind the UI within seconds, no restart — the directory is watched. |
| 📺 **Offline-friendly** | If TMDB is unreachable, your cached library still browses fine. |

The bundled UI is a single self-contained HTML document compiled into the binary — no
asset pipeline, no CDN, no build step. It talks to the same public `/api/v1` endpoints
as any other client, so you can point your own frontend at it instead.

## Configuration

Variable names and defaults match the original, so an existing MovieDB deployment can
be pointed at this image unchanged.

### Required

| Variable | Notes |
| --- | --- |
| `MOVIEDB_TMDB_APIKEY` | TMDB v3 key or v4 read access token. |
| `MOVIEDB_KEYCLOAK_ENABLED` | Required by the original. Auth is **not implemented** here — set `false`. `MOVIEDB_OAUTH2_ENABLED` is accepted as the 2.0 name. |

### Container

Follows the [LinuxServer.io](https://www.linuxserver.io/) conventions — though it is
neither affiliated with nor built on their base images.

| Variable | Default | Notes |
| --- | --- | --- |
| `PUID` | `911` | The container moves its own `abc` user onto this uid, so bind mounts work without you chowning anything. |
| `PGID` | `911` | As above, for the group. |
| `UMASK` | `022` | Widen to `002` for group-writable shares. |
| `TZ` | unset | Any tzdata name, e.g. `Europe/London`. |

Extras that come with it:

- **s6-overlay supervision** — if the server crashes, s6 restarts it rather than
  letting the container die.
- **`/custom-cont-init.d`** — mount executable scripts there to run as root before
  the service starts, without rebuilding the image.
- **Ownership is fixed only when needed** — the recursive chown is skipped when the
  top-level directory already has the right owner, so a 40k-poster cache doesn't get
  re-walked on every boot.

<details>
<summary>Paths and networking</summary>

| Variable | Default |
| --- | --- |
| `MOVIEDB_PORT` | `8081` |
| `MOVIEDB_DATABASE_PATH` | `/database` (`./database` outside Docker) |
| `MOVIEDB_IMAGE_CACHE_PATH` | `/imageCache` (`./imageCache` outside Docker) |
| `MOVIEDB_TMDB_LANGUAGE` | `en-US` |
| `MOVIEDB_TMDB_REGION` | `US` — picks which certification and streaming providers to show |

</details>

<details>
<summary>Display tuning</summary>

| Variable | Default |
| --- | --- |
| `MOVIEDB_LOW_RATING_THRESHOLD` | `40` |
| `MOVIEDB_HIGH_RATING_THRESHOLD` | `70` |
| `MOVIEDB_SHOW_MOVIE_CAST` | `true` (falls back to `MOVIEDB_SHOW_CAST`) |
| `MOVIEDB_SHOW_TV_CAST` | `true` (falls back to `MOVIEDB_SHOW_CAST`) |
| `MOVIEDB_SHOW_RECOMMENDATIONS` | `true` |
| `MOVIEDB_USE_MOVIEBACKGROUNDS` | `true` |
| `MOVIEDB_ADD_MEDIATYPE_HEADER_TO_VIDEOCARD` | `true` |
| `MOVIEDB_SUPPORT_DETAIL_CARDS` | `false` |
| `MOVIEDB_SHOW_TVSHOWS_IN_VIDEOLIST` | `true` |
| `MOVIEDB_SHOW_TVSEASONS_IN_VIDEOLIST` | `true` |
| `MOVIEDB_LIST_MAX_CARDS` | `200` |
| `MOVIEDB_LIST_MAX_LIGHT_CARDS` | `300` |
| `MOVIEDB_NUMBER_OF_RECOMMENDATIONS` | `12` |
| `MOVIEDB_NUMBER_OF_TOP_RECOMMENDATIONS` | `12` |
| `MOVIEDB_NUMBER_OF_MOVIE_CAST_REFERENCES` | `12` (falls back to `MOVIEDB_NUMBER_OF_CAST_REFERENCES`) |
| `MOVIEDB_NUMBER_OF_TV_CAST_REFERENCES` | `12` (same fallback) |
| `MOVIEDB_NUMBER_OF_DIRECTED_MOVIES` | `12` |
| `MOVIEDB_DEFAULT_MOBILE_POSTERWIDTH` | `133` |
| `MOVIEDB_DEFAULT_DESKTOP_POSTERWIDTH` | `220` |
| `MOVIEDB_SUBSCRIBED_WATCH_PROVIDERS` | empty — comma-separated provider names |

</details>

### Recognised but not implemented

`MOVIEDB_EMBY_*`, `MOVIEDB_INFLUXDB_*` and the OAuth2/Keycloak variables are parsed
and **logged as a warning at startup**, then ignored — they are not silently dropped,
but nothing acts on them. Also absent: the MySQL backend (SQLite only) and the
`werstreamt.es` integration, though `werStreamtEsId` round-trips through the API and
is stored.

## API

All 26 endpoints from the recovered spec live under `/api/v1`:

| Method | Path |
| --- | --- |
| `GET` | `/images?imagePath=&backdropSize=` |
| `GET` | `/images/wallpaper/{wallpaper}` |
| `GET` | `/tmdb/configuration` |
| `GET` `DELETE` `PATCH` | `/movie/{movieId}` |
| `GET` | `/movie/{movieId}/trailer` |
| `GET` | `/movie/{movieId}/backdrops` · `/posters` |
| `GET` | `/movie/credits/{movieId}` |
| `GET` | `/movie/recommendations/{movieId}` |
| `GET` | `/movies?tag=&not=` · `/movies/favorites` · `/movies/topRecommendations?limit=` |
| `GET` `PATCH` | `/person/{personId}` |
| `GET` | `/person/{personId}/profiles` |
| `GET` | `/search/tmdb?query=` |
| `GET` `DELETE` `PATCH` | `/tvshow/{tvShowId}` |
| `GET` | `/tvshow/{tvShowId}/backdrops` · `/posters` |
| `GET` | `/tvshows?tag=&not=` |
| `GET` `PATCH` | `/tvseason/{tvShowId}/{seasonId}` |
| `GET` | `/tvseason/{tvShowId}/{seasonId}/posters` |
| `PATCH` | `/tvepisode/{tvShowId}/{tvSeasonId}/{tvEpisodeId}` |
| `GET` | `/collection/{collectionId}` |
| `GET` | `/videos` |

`GET` endpoints accept `?refresh=true` to force a re-fetch; `/movie/{id}` and
`/tvshow/{id}` also accept `?loadDetails=true`.

Two additions beyond the original: `PATCH /api/v1/tvseason/{show}/{season}/watched`
(bulk-marks a season, which the original could only do episode by episode) and
`GET /health` + `/actuator/health` for container probes.

### Toggling state

```bash
# Add a tag
curl -X PATCH localhost:8081/api/v1/movie/603 \
  -H 'content-type: application/json' -d '{"tag":"4k","checked":true}'

# Mark watched — favorite / watched / onWatchlist are reserved tag names
curl -X PATCH localhost:8081/api/v1/movie/603 \
  -H 'content-type: application/json' -d '{"tag":"watched","checked":true}'

# Override the poster; a later ?refresh=true will not undo this
curl -X PATCH localhost:8081/api/v1/movie/603 \
  -H 'content-type: application/json' -d '{"poster_path":"/mine.jpg"}'
```

## Architecture

```
src/
  api/       axum handlers — paths mirror the recovered OpenAPI document exactly
  service/   get-or-fetch caching layer; the only place that decides store vs TMDB
  store/     Store trait + SQLite implementation (WAL mode)
  tmdb/      TMDB v3 client and response DTOs
  model/     wire types, field-for-field with the original's schemas
  web/       the bundled single-page UI
docker/root/ s6-overlay service definitions and init scripts
migrations/  SQL schema, applied automatically at startup
```

Three decisions worth knowing about:

**The `Store` trait.** All SQL lives in `store::sqlite`; nothing above it knows the
backend. Swapping engines means adding an implementation and repointing the
`ActiveStore` type alias.

**SQLite in WAL mode.** [Turso](https://github.com/tursodatabase/turso)'s Rust rewrite
was considered and rejected: its concurrency win comes from the MVCC engine, which is
still experimental and **does not support indexes** — and this schema leans on them.
WAL also suits the actual workload, which is read-heavy with bursty single-writer
inserts when a title is cached; the dominant latency is the TMDB round-trip, not write
contention.

**Overrides in separate columns.** `poster_path_override` and friends sit beside the
upstream values and are `COALESCE`d over them on read, which is what makes
`?refresh=true` always safe to run.

### Notes on fidelity

- `vote_average` is scaled to **0..=100** on videos, matching the original's
  `LOW_RATING_THRESHOLD=40` / `HIGH_RATING_THRESHOLD=70` defaults. Episodes keep
  TMDB's 0..=10 float, as the recovered schema specifies.
- The wire format reproduces the original's inconsistent casing exactly —
  `displayName` beside `poster_path`, `castId` on TV cast but `cast_id` on movie cast.
  Existing clients keep working.
- Absent fields are **omitted**, not sent as `null` (the original ran Jackson with
  `default-property-inclusion=non_null`).
- `VideoPatch` exposes only `tag` + `checked`, yet responses carry `favorite`,
  `watched` and `onWatchlist` booleans — so those three tag names are treated as
  reserved and routed to their columns instead of the tag table. **This is an
  inference**, not something the spec states.
- `Person.movieCast` / `directedMovies` / `tvCast` return `cast_id` = the **video's**
  id (the schema doesn't say which id it carries), with `character` holding the role
  or, for crew, the job. This makes the person page link into your library.

## Development

```bash
cargo test                              # 41 tests, no network or TMDB key needed
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo run
```

Tests run against an in-memory SQLite database and drive the real axum router, so they
cover routing, serialisation, patch semantics, bind-parameter chunking and the image
path guards without a TMDB key.

**Not covered by tests:** the TMDB fetch paths themselves. They need a live key, and
are exercised only indirectly (an invalid key correctly surfaces as a `502`).

### Container images

Published to GHCR on every push to `main`, and to Docker Hub when configured:

```
ghcr.io/royalflamejlh/tmdbcache:latest
ghcr.io/royalflamejlh/tmdbcache:v1.2.3     # on tags
ghcr.io/royalflamejlh/tmdbcache:sha-abc1234
```

Each architecture is built on a native runner (`ubuntu-latest` and
`ubuntu-24.04-arm`) and merged into one manifest list — building arm64 under QEMU
would turn a ~3 minute Rust build into a ~40 minute one.

To enable Docker Hub publishing, set:

| Kind | Name | Example |
| --- | --- | --- |
| Variable | `DOCKERHUB_REPOSITORY` | `youruser/tmdbcache` |
| Secret | `DOCKERHUB_USERNAME` | `youruser` |
| Secret | `DOCKERHUB_TOKEN` | a Docker Hub access token with **Read & Write** |

Until those exist the workflow still runs and publishes to GHCR, logging a notice.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short: `main` is protected, so open a PR,
and keep `cargo fmt`, `cargo clippy` and `cargo test` green.

## Acknowledgements

- **justsomebody42** for the original MovieDB, which this reimplements.
- **[LinuxServer.io](https://www.linuxserver.io/)** for the container conventions this
  copies. This project is not affiliated with them.
- **[s6-overlay](https://github.com/just-containers/s6-overlay)** for the supervision.

## License

[MIT](LICENSE) — with the exception of the recovered upstream documentation in
`docs/`, which is third-party material; see
[`docs/PROVENANCE.md`](docs/PROVENANCE.md).

This product uses the TMDB API but is not endorsed or certified by TMDB.
