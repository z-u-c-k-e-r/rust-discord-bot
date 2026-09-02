FROM rust:bookworm AS builder
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake libopus-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --locked || cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg libopus0 yt-dlp \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 zuckerbot
WORKDIR /app
COPY --from=builder /app/target/release/zuckerbot /usr/local/bin/zuckerbot
COPY scripts ./scripts
COPY web ./web
RUN mkdir -p /app/data && chown -R zuckerbot:zuckerbot /app
USER zuckerbot
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/zuckerbot"]
