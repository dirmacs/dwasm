# syntax=docker/dockerfile:1.7

# dwasm — build tool for Leptos WASM frontends (wasm-bindgen, wasm-opt,
# content hashing, index.html patching). Pure Rust: clap + sha2 + hex only,
# no system dependencies beyond libc.

# ---------- builder ----------
FROM rust:1.99-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# --locked: honor the committed lockfile; fail rather than re-resolve.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked && \
    cp /app/target/release/dwasm /tmp/dwasm

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --no-create-home --uid 10001 dwasm

WORKDIR /srv
COPY --from=builder /tmp/dwasm /usr/local/bin/dwasm

USER 10001

ENTRYPOINT ["dwasm"]
