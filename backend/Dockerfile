FROM rust:1-slim-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates ffmpeg && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/zaokaiy-api /usr/local/bin/zaokaiy-api
ENV BIND_ADDR=0.0.0.0:8080 MEDIA_DIR=/app/media
VOLUME /app/media
EXPOSE 8080
CMD ["zaokaiy-api"]
