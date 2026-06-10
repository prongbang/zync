FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p zync-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/zync-server /usr/local/bin/zync-server
ENV ZYNC_BIND=0.0.0.0:58271
EXPOSE 58271
CMD ["zync-server"]

