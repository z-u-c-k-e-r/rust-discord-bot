FROM rust:1.88-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake pkg-config libopus-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release --workspace

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg libopus0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/zuckerbot-api /usr/local/bin/zuckerbot-api
COPY --from=builder /app/target/release/zuckerbot-bot /usr/local/bin/zuckerbot-bot
COPY plugins /app/plugins

RUN useradd --create-home --uid 10001 zuckerbot \
    && chown -R zuckerbot:zuckerbot /app
USER zuckerbot

EXPOSE 8080
CMD ["/usr/local/bin/zuckerbot-api"]
