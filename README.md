# tmdbcache

[![CI](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/ci.yml/badge.svg)](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/ci.yml)
[![Docker](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/docker.yml/badge.svg)](https://github.com/Royalflamejlh/tmdbcache/actions/workflows/docker.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A self-hosted catalogue for your movie and TV collection. Metadata comes from TMDB
and is cached in SQLite; artwork is cached on disk. Once a title has been fetched,
looking at it costs no network at all.

This is a Rust rewrite of MovieDB, a Spring Boot app that used to live at
`justsomebody42/movieDB`. The source is gone from GitHub, but the
[container image](https://hub.docker.com/r/moviedb/moviedb) is still up and it shipped
its own OpenAPI document. The API here matches that document, so anything written
against the original still works. It's checked in at
[`docs/openapi-original.yaml`](docs/openapi-original.yaml) if you want to compare.

> [!WARNING]
> There is no authentication. Anyone who can reach the port can read your library,
> add and delete titles, and spend your TMDB quota. Put it behind a reverse proxy or
> keep it on a trusted network.

## Quick start

```yaml
---
services:
  tmdbcache:
    image: ghcr.io/royalflamejlh/tmdbcache:latest
    container_name: tmdbcache
    environment:
      - MOVIEDB_TMDB_APIKEY=your_tmdb_key
      - MOVIEDB_KEYCLOAK_ENABLED=false
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

Open <http://localhost:8081>, search for something, and click a result. Opening a
search result is what pulls it into your library.

Images are published for amd64 and arm64.

Running it without Docker works too, you just need the two required variables set:

```bash
export MOVIEDB_TMDB_APIKEY=your_key
export MOVIEDB_KEYCLOAK_ENABLED=false
cargo run --release
```

Either a TMDB v3 API key or a v4 read access token will do. The key's shape is
detected and sent as `api_key` or as a bearer token to match.

## What you get

Search TMDB and add movies or TV shows. Everything about a title is cached locally,
so pages render instantly and your library still browses fine when TMDB is down or
your key has run out of quota.

On top of the cached metadata you can mark things watched, favourite, or on a
watchlist, and attach arbitrary tags. All of that works per movie, per show, per
season, and per episode.

You can also override a poster, backdrop, overview, or IMDb id by hand. Overrides are
kept in their own columns and layered over the upstream values on read, so
`?refresh=true` won't undo your edits.

The Discover page pools the TMDB recommendations of everything you own and ranks the
titles you don't have by how often they come up.

Drop `.jpg` or `.png` files into `imageCache/wallpapers` and they'll show up behind
the UI within a few seconds. The directory is watched, so no restart.

The bundled UI is one self-contained HTML file compiled into the binary. No asset
pipeline, no CDN, nothing to mount. It uses the same public `/api/v1` endpoints as
any other client, so you can point your own frontend at it and ignore it entirely.

## Configuration

Variable names and defaults are the same as the original, so an existing MovieDB
deployment can be pointed at this image without changes.

Two are required:

| Variable | Notes |
| --- | --- |
| `MOVIEDB_TMDB_APIKEY` | TMDB v3 key or v4 read access token. |
| `MOVIEDB_KEYCLOAK_ENABLED` | The original required this. Auth isn't implemented here, so set `false`. `MOVIEDB_OAUTH2_ENABLED` also works. |

### Container

| Variable | Default | Notes |
| --- | --- | --- |
| `PUID` | `911` | The container moves its own service user onto this uid at startup, which means bind mounts work without chowning anything first. |
| `PGID` | `911` | Same, for the group. |
| `UMASK` | `022` | Set `002` for group-writable shares. |
| `TZ` | unset | Any tzdata name, e.g. `Europe/London`. |

The server runs under s6-overlay rather than as PID 1, so if it crashes it gets
restarted instead of taking the container down with it. Executable scripts mounted at
`/custom-cont-init.d` run as root before the server starts, which is handy for
one-off setup you don't want to rebuild the image for.

The startup chown is skipped when the data directories already have the right owner.
An image cache with tens of thousands of posters in it doesn't get re-walked on every
boot.

<details>
<summary>Paths and networking</summary>

| Variable | Default |
| --- | --- |
| `MOVIEDB_PORT` | `8081` |
| `MOVIEDB_DATABASE_PATH` | `/database` (`./database` outside Docker) |
| `MOVIEDB_IMAGE_CACHE_PATH` | `/imageCache` (`./imageCache` outside Docker) |
| `MOVIEDB_TMDB_LANGUAGE` | `en-US` |
| `MOVIEDB_TMDB_REGION` | `US`, picks which age certification and streaming providers get shown |

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
| `MOVIEDB_SUBSCRIBED_WATCH_PROVIDERS` | empty, comma-separated provider names |

</details>

## API

Everything lives under `/api/v1`.

| Method | Path |
| --- | --- |
| `GET` | `/images?imagePath=&backdropSize=` |
| `GET` | `/images/wallpaper/{wallpaper}` |
| `GET` | `/tmdb/configuration` |
| `GET` `DELETE` `PATCH` | `/movie/{movieId}` |
| `GET` | `/movie/{movieId}/trailer` |
| `GET` | `/movie/{movieId}/backdrops`, `/posters` |
| `GET` | `/movie/credits/{movieId}` |
| `GET` | `/movie/recommendations/{movieId}` |
| `GET` | `/movies?tag=&not=`, `/movies/favorites`, `/movies/topRecommendations?limit=` |
| `GET` `PATCH` | `/person/{personId}` |
| `GET` | `/person/{personId}/profiles` |
| `GET` | `/search/tmdb?query=` |
| `GET` `DELETE` `PATCH` | `/tvshow/{tvShowId}` |
| `GET` | `/tvshow/{tvShowId}/backdrops`, `/posters` |
| `GET` | `/tvshows?tag=&not=` |
| `GET` `PATCH` | `/tvseason/{tvShowId}/{seasonId}` |
| `GET` | `/tvseason/{tvShowId}/{seasonId}/posters` |
| `PATCH` | `/tvepisode/{tvShowId}/{tvSeasonId}/{tvEpisodeId}` |
| `GET` | `/collection/{collectionId}` |
| `GET` | `/videos` |

`GET` endpoints take `?refresh=true` to force a re-fetch. `/movie/{id}` and
`/tvshow/{id}` also take `?loadDetails=true`, which pulls credits and recommendations
in the same round trip.

Beyond the original there's `PATCH /api/v1/tvseason/{show}/{season}/watched` for
marking a whole season at once, and `/health` for container probes.

State is changed by patching a tag. `favorite`, `watched`, and `onWatchlist` are
reserved names that map onto their own columns; anything else is a free-form tag.

```bash
# add a tag
curl -X PATCH localhost:8081/api/v1/movie/603 \
  -H 'content-type: application/json' -d '{"tag":"4k","checked":true}'

# mark watched
curl -X PATCH localhost:8081/api/v1/movie/603 \
  -H 'content-type: application/json' -d '{"tag":"watched","checked":true}'

# override the poster; a later ?refresh=true won't undo this
curl -X PATCH localhost:8081/api/v1/movie/603 \
  -H 'content-type: application/json' -d '{"poster_path":"/mine.jpg"}'
```

## Layout

```
src/
  api/       axum handlers, one route per path in the OpenAPI document
  service/   the get-or-fetch layer between the handlers and the store or TMDB
  store/     Store trait plus the SQLite implementation
  tmdb/      TMDB v3 client and response types
  model/     wire types
  web/       the bundled UI
docker/root/ s6 service definitions and init scripts
migrations/  schema, applied at startup
```

All SQL lives in `store::sqlite`, behind a `Store` trait. SQLite runs in WAL mode.

Three things to know if you're writing a client. Ratings on videos are integers from
0 to 100, which is where the `40` and `70` threshold defaults come from, while episode
ratings are TMDB's 0 to 10 float. Field naming is inconsistent in the same places the
original's was (`displayName` next to `poster_path`, `castId` on TV cast but `cast_id`
on movie cast). Missing values are left out of responses rather than sent as `null`.

## Development

```bash
cargo test                              # 41 tests, no network or TMDB key needed
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo run
```

Tests use an in-memory database and drive the real router, covering routing,
serialisation, patch behaviour, and the image path guards. The TMDB fetch paths
themselves aren't tested, since they need a live key.

`main` is protected and CI is a required check, so open a PR. See
[CONTRIBUTING.md](CONTRIBUTING.md).

### Images

Built on every push to `main` and tagged `latest`, `main`, and `sha-<short>`. Tagging
a release as `v1.2.3` also publishes the semver tags. amd64 and arm64 each build on a
runner of their own architecture, then get merged into one manifest.

Publishing to GHCR needs no setup. Docker Hub is skipped unless these are set:

| Kind | Name | Example |
| --- | --- | --- |
| Variable | `DOCKERHUB_REPOSITORY` | `youruser/tmdbcache` |
| Secret | `DOCKERHUB_USERNAME` | `youruser` |
| Secret | `DOCKERHUB_TOKEN` | an access token with Read & Write |

## Roadmap

Not built yet, roughly in the order I'd get to them.

- **Authentication.** There is none right now. See [SECURITY.md](SECURITY.md) for how
  to deploy around it in the meantime.
- **Other database backends.** SQLite today. MySQL and PostgreSQL both fit behind the
  existing `Store` trait, and MySQL is what the original offered as the alternative.
- **Emby sync.** The original could read library state from an Emby server.
- **InfluxDB metrics.** The original could publish local REST and TMDB API usage for
  graphing.
- **werstreamt.es lookups.** `werStreamtEsId` is stored and returned by the API
  already, but nothing reads it.

The `MOVIEDB_EMBY_*`, `MOVIEDB_INFLUXDB_*` and OAuth2 variables are parsed, and
setting any of them logs a warning at startup so a configured deployment isn't left
guessing.

## Credits

Thanks to justsomebody42 for the original MovieDB, and to
[s6-overlay](https://github.com/just-containers/s6-overlay) for the process
supervision.

## License

[MIT](LICENSE).

This product uses the TMDB API but is not endorsed or certified by TMDB.
