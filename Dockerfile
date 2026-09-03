ARG RUST_VERSION=1.98.1
ARG DEBIAN_SUITE=trixie

FROM rust:${RUST_VERSION}-${DEBIAN_SUITE} AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential \
        cmake \
        libopus-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml rust-toolchain.toml ./
COPY src ./src
COPY migrations ./migrations
COPY scripts ./scripts
COPY web ./web

RUN cargo generate-lockfile \
    && cargo build --locked --release

FROM debian:${DEBIAN_SUITE}-slim AS runtime

ARG YT_DLP_VERSION=2026.08.19
ARG YT_DLP_SHA256=1fa6733c37ea6fb51c99ad8fe785e7b7e5f3246c9b980230329d4fb72ed8d4d6

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        curl \
        ffmpeg \
        libopus0 \
        python3 \
    && curl --fail --location --proto '=https' --tlsv1.2 \
        "https://github.com/yt-dlp/yt-dlp/releases/download/${YT_DLP_VERSION}/yt-dlp" \
        --output /usr/local/bin/yt-dlp \
    && echo "${YT_DLP_SHA256}  /usr/local/bin/yt-dlp" | sha256sum --check --strict \
    && chmod 0755 /usr/local/bin/yt-dlp \
    && yt-dlp --version \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin zuckerbot

WORKDIR /app

COPY --from=builder /app/target/release/zuckerbot /usr/local/bin/zuckerbot
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/scripts ./scripts
COPY --from=builder /app/web ./web

USER zuckerbot

EXPOSE 8080

CMD ["zuckerbot"]
