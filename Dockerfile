# syntax=docker/dockerfile:1.4
# =============================================================================
# neutrino-notes — standalone Dockerfile
#
# Build:
#   docker build -t neutrino-notes .
#
# NOTE: 'shared' is fetched from GitHub during build. For private repos you
# need to provide SSH credentials at build time:
#   DOCKER_BUILDKIT=1 docker build --ssh default -t neutrino-notes .
# =============================================================================

# ── Build Stage ──────────────────────────────────────────────────────────────
FROM rust:1.95 AS builder

WORKDIR /app

COPY Cargo.toml ./
COPY Cargo.lock* ./

RUN mkdir src && echo "fn main(){}" > src/main.rs

RUN cargo fetch
RUN cargo build --release
RUN rm -rf src

# Build the real binary
COPY src src
COPY migrations migrations
RUN touch src/main.rs && cargo build --release

# ── Runtime Stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN useradd -m appuser

WORKDIR /app

RUN mkdir -p /usr/local/data /usr/local/logs \
    && chown -R appuser:appuser /usr/local/data /usr/local/logs

RUN apt-get update \
    && apt-get install -y openssl libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/neutrino-notes /usr/local/bin/service

USER appuser

EXPOSE 8080

VOLUME ["/usr/local/data", "/usr/local/logs"]

CMD ["/usr/local/bin/service"]
