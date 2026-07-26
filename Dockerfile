FROM oven/bun:1 AS web-builder
WORKDIR /app/web
COPY web/ .
RUN bun install --frozen-lockfile || bun install
RUN cd apps/web && bun run build

FROM rust:1-bookworm AS server-builder
WORKDIR /app
COPY . .
COPY --from=web-builder /app/web/apps/web/dist /app/web/apps/web/dist
RUN cargo build --release -p zync-server --features embed-ui

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client \
    && rm -rf /var/lib/apt/lists/*

# Run as a non-root system user rather than uid 0. The server shells out to
# git/ssh on user-registered paths, so dropping root limits the blast radius of
# any RCE in that surface. The data (/data → zync.db) and workspaces
# (/workspaces) dirs are created and chowned to this user so that the *named*
# volumes mounted there inherit its ownership (Docker copies the image dir's
# ownership onto an empty named volume). Note: a host bind-mount at /workspaces
# keeps its host ownership, so align it with uid 10001 (or set `user:` in
# compose) if the server must write to a bind-mounted repo.
RUN groupadd --system --gid 10001 zync \
    && useradd --system --uid 10001 --gid zync --home-dir /app --no-create-home zync

WORKDIR /app
COPY --from=server-builder /app/target/release/zync /usr/local/bin/zync
RUN mkdir -p /data /workspaces \
    && chown -R zync:zync /app /data /workspaces
ENV ZYNC_BIND=0.0.0.0:58271
USER zync
EXPOSE 58271
CMD ["zync", "serve"]
