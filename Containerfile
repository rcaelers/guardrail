##
## Planner — extract dependency recipe from Cargo.toml/Cargo.lock
## This stage is tiny and only re-runs when the workspace manifests change.
##

FROM --platform=$BUILDPLATFORM rust:alpine3.23 AS planner

RUN apk add --no-cache musl-dev build-base && \
    cargo install cargo-chef --locked

WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

##
## Cacher — pre-build all third-party dependencies
## This whole stage is a cached Docker layer: it only re-runs when
## Cargo.lock changes, which means a dependency version changed.
##

FROM --platform=$BUILDPLATFORM rust:alpine3.23 AS cacher

ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}

RUN apk add --no-cache \
    clang \
    lld \
    openssl \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    musl-dev \
    build-base && \
    cargo install cargo-chef --locked

WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json

##
## Builder — compile all service binaries in one pass
## Shared workspace crates (common, repos, data, …) are compiled once.
## Only workspace crate source changes trigger a rebuild here.
##

FROM --platform=$BUILDPLATFORM rust:alpine3.23 AS builder

ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}

RUN apk add --no-cache \
    clang \
    lld \
    openssl \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    musl-dev \
    build-base

WORKDIR /app

COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY . .

RUN cargo build \
    -p api \
    -p web \
    -p ingestion \
    -p processor \
    -p curator && \
    cp target/debug/api       /app/api-bin && \
    cp target/debug/web       /app/web-bin && \
    cp target/debug/ingestion /app/ingestion-bin && \
    cp target/debug/processor /app/processor-bin && \
    cp target/debug/curator   /app/curator-bin

##
## Shared runtime base — Alpine with just the essentials
##

FROM alpine:latest AS base

RUN apk add --no-cache openssl ca-certificates tzdata tini && \
    rm -rf /var/cache/apk/*

RUN addgroup -g 1000 -S guardrail && adduser -u 1000 -S guardrail -G guardrail

WORKDIR /app

ENTRYPOINT ["/sbin/tini", "--"]

##
## Runtime: api
##

FROM base AS api

COPY --from=builder /app/api-bin /app/api

RUN mkdir -p /config /tmp/guardrail && \
    chown -R guardrail:guardrail /app /config /tmp/guardrail

VOLUME ["/config", "/tmp/guardrail"]

USER guardrail

CMD ["/app/api", "-C", "/config"]

##
## Runtime: ingestion
##

FROM base AS ingestion

COPY --from=builder /app/ingestion-bin /app/ingestion

RUN mkdir -p /config /tmp/guardrail && \
    chown -R guardrail:guardrail /app /config /tmp/guardrail

VOLUME ["/config", "/tmp/guardrail"]

USER guardrail

CMD ["/app/ingestion", "-C", "/config"]

##
## Runtime: processor
##

FROM base AS processor

COPY --from=builder /app/processor-bin /app/processor

RUN mkdir -p /config /tmp/guardrail && \
    chown -R guardrail:guardrail /app /config /tmp/guardrail

VOLUME ["/config", "/tmp/guardrail"]

USER guardrail

CMD ["/app/processor", "-C", "/config"]

##
## Runtime: curator
##

FROM base AS curator

COPY --from=builder /app/curator-bin /app/curator

RUN mkdir -p /config /tmp/guardrail && \
    chown -R guardrail:guardrail /app /config /tmp/guardrail

VOLUME ["/config", "/tmp/guardrail"]

USER guardrail

CMD ["/app/curator", "-C", "/config"]

##
## Runtime: web
##

FROM base AS web

COPY --from=builder /app/web-bin /app/web
COPY src/web/server/static /app/static

RUN mkdir -p /config && \
    chown -R guardrail:guardrail /app /config

VOLUME ["/config"]

USER guardrail

CMD ["/app/web", "-C", "/config"]
