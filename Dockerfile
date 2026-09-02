FROM rust:1.88-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential \
        cmake \
        libopus-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY src ./src
COPY migrations ./migrations
COPY scripts ./scripts
COPY web ./web

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        ffmpeg \
        libopus0 \
        yt-dlp \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 zuckerbot

WORKDIR /app

COPY --from=builder /app/target/release/zuckerbot /usr/local/bin/zuckerbot
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/scripts ./scripts
COPY --from=builder /app/web ./web

USER zuckerbot

EXPOSE 8080

CMD ["zuckerbot"]
