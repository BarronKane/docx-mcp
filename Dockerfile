FROM rust:1.89-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml README.md LICENSE.md ./
COPY crates ./crates

RUN cargo build -p docx-mcpd --release --locked

FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="docx-mcp" \
    org.opencontainers.image.description="Docx MCP daemon" \
    org.opencontainers.image.licenses="MIT" \
    org.opencontainers.image.source="https://github.com/BarronKane/docx-mcp"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /data

ENV DOCX_MCP_HTTP_ADDR=[::]:4020 \
    DOCX_INGEST_ADDR=[::]:4010 \
    DOCX_ENABLE_STDIO=1 \
    DOCX_MCP_SERVE=0 \
    DOCX_INGEST_SERVE=0 \
    DOCX_DB_IN_MEMORY=0

EXPOSE 4020 4010

COPY --from=builder /app/target/release/docx-mcpd /usr/local/bin/docx-mcpd
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]