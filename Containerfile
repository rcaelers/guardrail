##
## Chef — Rust build image with cargo-chef installed
##

FROM --platform=$BUILDPLATFORM rust:alpine3.23 AS chef

ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}

RUN apk add --no-cache \
    clang \
    mold \
    openssl \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    musl-dev \
    build-base

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo install cargo-chef --locked

WORKDIR /app

##
## Planner — extract dependency recipe from Cargo.toml/Cargo.lock
##

FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

##
## Builder — pre-build dependencies, then compile all binaries
##

FROM chef AS builder

ARG CARGO_BUILD_FLAGS=""
ARG CARGO_BUILD_OUTPUT_DIR=debug
ARG CARGO_BUILD_RUSTFLAGS=""
ENV RUSTFLAGS=${CARGO_BUILD_RUSTFLAGS}

COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/rust-toolchain.toml rust-toolchain.toml
COPY --from=planner /app/.cargo/config.toml .cargo/config.toml
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook ${CARGO_BUILD_FLAGS} --recipe-path recipe.json
COPY . .

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo build ${CARGO_BUILD_FLAGS} -p guardrail -p web --bin guardrail --bin guardrailctl && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/guardrail    /app/guardrail-bin && \
    cp target/${CARGO_BUILD_OUTPUT_DIR}/guardrailctl /app/guardrailctl-bin

##
## SurrealKit — download pre-built binary via cargo-binstall
##

FROM --platform=$BUILDPLATFORM alpine:3.23 AS surrealkit-downloader

RUN apk add --no-cache ca-certificates curl

RUN curl -L --proto '=https' --tlsv1.2 -sSf \
    https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
    | sh

RUN /root/.cargo/bin/cargo-binstall --no-confirm --version 0.5.8 surrealkit

##
## Runtime: server — unified image for all backend roles
##

FROM alpine:latest AS server

RUN apk add --no-cache openssl ca-certificates tzdata tini && \
    rm -rf /var/cache/apk/*

RUN addgroup -g 1000 -S guardrail && adduser -u 1000 -S guardrail -G guardrail

WORKDIR /app

COPY --from=builder /app/guardrail-bin    /app/guardrail
COPY --from=builder /app/guardrailctl-bin /app/guardrailctl
COPY src/web/server/static                /app/static

RUN mkdir -p /config /tmp/guardrail && \
    chown -R guardrail:guardrail /app /config /tmp/guardrail

VOLUME ["/config", "/tmp/guardrail"]

USER guardrail

ENTRYPOINT ["/sbin/tini", "--"]

CMD ["/app/guardrail", "--help"]

##
## Runtime: schema sync
##

FROM alpine:latest AS schema-sync

RUN apk add --no-cache openssl ca-certificates tzdata tini && \
    rm -rf /var/cache/apk/*

RUN addgroup -g 1000 -S guardrail && adduser -u 1000 -S guardrail -G guardrail

WORKDIR /app

COPY --from=surrealkit-downloader /root/.cargo/bin/surrealkit /usr/local/bin/surrealkit
COPY database /app/database

RUN chown -R guardrail:guardrail /app

USER guardrail

ENTRYPOINT ["/sbin/tini", "--"]

CMD ["surrealkit", "status"]
