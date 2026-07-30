# Provenance of the recovered specification

The upstream project, `justsomebody42/movieDB`, is gone from GitHub. Its published
container image — [`moviedb/moviedb`](https://hub.docker.com/r/moviedb/moviedb) —
is still on Docker Hub, and the Spring Boot application inside it ships an exploded
classpath rather than a fat jar.

## How the spec was recovered

```bash
skopeo copy docker://docker.io/moviedb/moviedb:2.0.0-SNAPSHOT dir:./img
# extract the layer tarballs, then:
#   app/resources/openapi/api-docs-bundle.yaml   -> docs/openapi-original.yaml
#   app/resources/application*.properties        -> configuration defaults
#   app/classes/moviedb/**                       -> TMDB endpoint strings
```

`api-docs-bundle.yaml` is the application's own bundled OpenAPI 3.0.3 document:
26 paths and roughly 40 schemas. It is reproduced here verbatim as
[`openapi-original.yaml`](openapi-original.yaml).

The TMDB request shapes were read out of the compiled classes with `strings`, which
is where the `append_to_response` combinations came from:

| TMDB call | Purpose |
| --- | --- |
| `movie/{id}?append_to_response=videos,external_ids,credits,recommendations` | one-shot movie fetch |
| `movie/{id}/release_dates` | age certification |
| `movie/{id}/watch/providers` | streaming availability |
| `tv/{id}?append_to_response=videos,external_ids,credits,recommendations` | one-shot show fetch |
| `tv/{id}/season/{n}` | episodes |
| `person/{id}?append_to_response=movie_credits,tv_credits` | person detail |
| `search/movie`, `search/tv`, `collection/{id}`, `configuration` | the rest |

## Why these files are included

They are the only surviving description of the interface this project reimplements,
and they are what makes the port auditable — you can diff the routes in `src/api/`
against `openapi-original.yaml` and see that nothing was invented.

## Licensing note

`openapi-original.yaml` and `original-dockerhub-readme.md` are **not** original
work of this project and are **not** covered by its MIT licence. They are
third-party material reproduced for interface documentation and attribution:

- The original project stated its licence only as "See: LICENSE file", and that
  file did not survive in the container image — so the upstream terms are unknown.
- All Rust code in this repository was written from scratch. No upstream Java
  source was available to copy, and none was decompiled.
- An API's structure — endpoint names, field names, types — is an interface
  description rather than creative expression.

If you are the original author and would prefer these files removed, please open an
issue and they will be taken out; the port itself does not depend on them at build
or run time.
