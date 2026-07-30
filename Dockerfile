# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
# SQLite is statically bundled (the `sqlite-bundled` feature), so the runtime
# image needs no libsqlite3.
#
# There is no cross-compilation here: CI builds each architecture on a native
# runner, which is far faster than emulating one under QEMU.
FROM docker.io/library/rust:1-slim-bookworm AS build

WORKDIR /src

# Warm the dependency cache before copying sources so edits under src/ do not
# invalidate a full dependency rebuild.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY migrations ./migrations
COPY src ./src
# Touch the entry points so cargo rebuilds them over the placeholder artifacts.
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --locked \
    && strip target/release/tmdbcache


# ---------------------------------------------------------------------------
# Runtime
# ---------------------------------------------------------------------------
FROM docker.io/library/debian:bookworm-slim AS runtime

# s6-overlay supervises the server so a crash restarts it instead of taking the
# container down. It also provides the ordered init scripts under docker/root/.
ARG S6_OVERLAY_VERSION=3.2.1.0
# Set by buildx; maps onto s6-overlay's release asset names.
ARG TARGETARCH

# ca-certificates: TLS to api.themoviedb.org
# tzdata:          honours the TZ environment variable
# xz-utils:        unpacks the s6-overlay release tarballs (removed afterwards)
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        tzdata \
        xz-utils; \
    case "${TARGETARCH}" in \
        amd64)  S6_ARCH=x86_64 ;; \
        arm64)  S6_ARCH=aarch64 ;; \
        arm)    S6_ARCH=armhf ;; \
        386)    S6_ARCH=i686 ;; \
        *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    cd /tmp; \
    ADDR="https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}"; \
    apt-get install -y --no-install-recommends curl; \
    curl -fsSLO "${ADDR}/s6-overlay-noarch.tar.xz"; \
    curl -fsSLO "${ADDR}/s6-overlay-${S6_ARCH}.tar.xz"; \
    tar -C / -Jxpf s6-overlay-noarch.tar.xz; \
    tar -C / -Jxpf "s6-overlay-${S6_ARCH}.tar.xz"; \
    rm -f /tmp/*.tar.xz; \
    apt-get purge -y --auto-remove xz-utils curl; \
    rm -rf /var/lib/apt/lists/*

# The service user. init-adduser moves it onto the operator's PUID/PGID at boot,
# so the build-time ids here only matter until then.
RUN set -eux; \
    groupadd --gid 911 abc; \
    useradd --uid 911 --gid abc --shell /bin/false --no-create-home abc

COPY --from=build /src/target/release/tmdbcache /usr/local/bin/tmdbcache
# The s6 service definitions, init scripts and the chown-if-root helper.
COPY docker/root/ /

ARG VERSION=dev
ARG BUILD_DATE=unknown

ENV MOVIEDB_DATABASE_PATH=/database \
    MOVIEDB_IMAGE_CACHE_PATH=/imageCache \
    MOVIEDB_PORT=8081 \
    TMDBCACHE_VERSION=${VERSION} \
    TMDBCACHE_BUILD_DATE=${BUILD_DATE} \
    # Abort the container if an init script fails, rather than starting the
    # server with surprising ownership or configuration.
    S6_BEHAVIOUR_IF_STAGE2_FAILS=2 \
    # The oneshots are ordered by dependency, so no global wait is needed.
    S6_CMD_WAIT_FOR_SERVICES_MAXTIME=0 \
    S6_VERBOSITY=1

VOLUME ["/database", "/imageCache"]
EXPOSE 8081

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/tmdbcache", "--healthcheck"]

LABEL org.opencontainers.image.title="tmdbcache" \
      org.opencontainers.image.description="Self-hosted TMDB-backed video library, a Rust reimplementation of MovieDB" \
      org.opencontainers.image.source="https://github.com/Royalflamejlh/tmdbcache" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.created="${BUILD_DATE}"

ENTRYPOINT ["/init"]
