FROM rust:1-slim-bookworm AS build
WORKDIR /app
# 1) dependencies only (cached until a Cargo.toml / Cargo.lock changes)
COPY Cargo.toml Cargo.lock* ./
COPY crates/kernel/Cargo.toml crates/kernel/
COPY crates/platform/Cargo.toml crates/platform/
COPY crates/commerce/Cargo.toml crates/commerce/
COPY crates/vehicle/Cargo.toml crates/vehicle/
COPY crates/restaurant/Cargo.toml crates/restaurant/
COPY crates/insurance/Cargo.toml crates/insurance/
COPY crates/finance/Cargo.toml crates/finance/
COPY crates/gateway/Cargo.toml crates/gateway/
RUN for c in kernel platform commerce vehicle restaurant insurance finance; do mkdir -p crates/$c/src && touch crates/$c/src/lib.rs; done \
 && mkdir -p crates/gateway/src && echo 'fn main(){}' > crates/gateway/src/main.rs \
 && cargo build --release -p gateway \
 && rm -rf crates/*/src
# 2) the workspace (sqlx::migrate! embeds each crate's migrations/ at compile time)
COPY crates ./crates
RUN find crates -name '*.rs' -exec touch {} + && cargo build --release -p gateway

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates ffmpeg && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/zaokaiy-api /usr/local/bin/zaokaiy-api
# ZK_CORES=all runs every core in this container; see README "Super app: cores" to split them.
ENV BIND_ADDR=0.0.0.0:8080 MEDIA_DIR=/app/media PRIVATE_DIR=/app/private ZK_CORES=all
VOLUME ["/app/media", "/app/private"]
EXPOSE 8080
CMD ["zaokaiy-api"]
