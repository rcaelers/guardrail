##
## Chef — Rust build image with cargo-chef installed
##

FROM --platform=$BUILDPLATFORM rust:alpine3.23 AS chef

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

##
## Planner — extract dependency recipe from Cargo.toml/Cargo.lock
##

FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

##
## Builder — pre-build dependencies, then compile service binaries
##

FROM chef AS builder

ARG CARGO_BUILD_FLAGS=""
ARG CARGO_BUILD_OUTPUT_DIR=debug
ARG CARGO_BUILD_RUSTFLAGS=""
ENV RUSTFLAGS=${CARGO_BUILD_RUSTFLAGS}

COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/rust-toolchain.toml rust-toolchain.toml
COPY --from=planner /app/.cargo/config.toml .cargo/config.toml
RUN cargo chef cook ${CARGO_BUILD_FLAGS} --recipe-path recipe.json
COPY . .

RUN cargo build ${CARGO_BUILD_FLAGS} \
    -p api \
    -p web \
    -p ingestion \
    -p processor \
    -p curator && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/api          /app/api-bin && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/web          /app/web-bin && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/guardrailctl /app/guardrailctl-bin && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/ingestion    /app/ingestion-bin && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/processor    /app/processor-bin && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/curator      /app/curator-bin

##
## SurrealKit — schema management CLI
##

FROM chef AS surrealkit-builder

ENV RUSTUP_TOOLCHAIN=stable

RUN cargo +stable install --locked --version 0.5.6 surrealkit

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
COPY --from=builder /app/guardrailctl-bin /app/guardrailctl
COPY src/web/server/static /app/static

RUN mkdir -p /config && \
    chown -R guardrail:guardrail /app /config

VOLUME ["/config"]

USER guardrail

CMD ["/app/web", "-C", "/config"]

##
## Runtime: schema sync
##

FROM base AS schema-sync

COPY --from=surrealkit-builder /usr/local/cargo/bin/surrealkit /usr/local/bin/surrealkit
COPY database /app/database

RUN chown -R guardrail:guardrail /app

USER guardrail

CMD ["surrealkit", "status"]
