FROM oven/bun:1 AS web-builder
WORKDIR /app/web
COPY web/ .
RUN bun install --frozen-lockfile || bun install
RUN cd apps/web && bun run build

FROM rust:1-bookworm AS server-builder
WORKDIR /app
COPY . .
RUN cargo build --release -p zync-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server-builder /app/target/release/zync-server /usr/local/bin/zync-server
COPY --from=web-builder /app/web/apps/web/dist /app/public
ENV ZYNC_BIND=0.0.0.0:58271
ENV ZYNC_STATIC_DIR=/app/public
EXPOSE 58271
CMD ["zync-server"]
