# verify-server — container image (S2/S3 replay-verify server).
# Build context = repo ROOT (the Cargo workspace). Builds ONLY verify-server + its
# deps (sim-core); godot-ext is not in verify-server's dep graph so it isn't compiled.

# ---- build stage ----
# slim-bookworm = latest stable Rust on Debian bookworm (edition2024-capable) — the
# bookworm base keeps glibc identical to the runtime stage (bookworm-slim) so the
# dynamically-linked binary runs. (1.83 was too old: a dep needs edition2024.)
FROM rust:slim-bookworm AS build
WORKDIR /build
# Git identity of the source being shipped (.dockerignore strips .git/, so the
# deploy script passes it: fly deploy --build-arg GIT_SHA=$(git rev-parse --short=12 HEAD)).
# sim-core's build.rs bakes it into the binary; /version reports it for sync checks.
ARG GIT_SHA=unknown
ENV SIM_GIT_SHA=$GIT_SHA
# Copy the workspace. (For faster rebuilds you could split deps vs src; kept simple.)
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# data/ is needed by the sim-core golden tests (include_str! game_data.json) below.
COPY data ./data
# GOLDEN GUARD (2026-07-10): run sim-core's deterministic goldens on THIS build's target (x86_64
# Linux) BEFORE building the server. The golden hashes are blessed on the dev machine (arm64
# macOS); if any sim change made the state hash differ CROSS-PLATFORM, a golden fails here and the
# deploy aborts — instead of the client (arm64) and server (x86) silently diverging so every online
# save is rejected (which is exactly how a combat change slipped through: goldens only ran on arm64).
# CACHE MOUNTS (2026-07-17, deploy speed): registry + target persist on the Fly builder across
# deploys, so a source-only change recompiles just the workspace crates incrementally instead of
# every dependency from scratch (was ~10-15 min/deploy). Cache mounts aren't image layers, so the
# binary must be copied OUT within the same RUN. Cache eviction = cold build, same as before.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo test --release -p sim-core --tests \
 && cargo build --release -p verify-server \
 && cp target/release/verify-server /verify-server-out

# ---- runtime stage ----
FROM debian:bookworm-slim
# ca-certificates: for future outbound HTTPS (Steam Web API). tini: clean signal handling.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /verify-server-out /app/verify-server
COPY data/game_data.json /app/data/game_data.json
# Commander reward config — read at boot for the level-reward CREDIT; a missing file is a boot panic.
COPY data/commander_rewards.json /app/data/commander_rewards.json
# Bind to all interfaces on the platform port (Fly default 8080); load the catalog.
ENV BIND=0.0.0.0:8080
ENV GAME_DATA_PATH=/app/data/game_data.json
ENV COMMANDER_REWARDS_PATH=/app/data/commander_rewards.json
EXPOSE 8080
CMD ["/app/verify-server"]
