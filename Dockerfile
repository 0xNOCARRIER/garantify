# ---------------------------------------------------------------
# Stage 1 : Compilation
# Runs via QEMU on the target arch. native-tls (OpenSSL)
# cross-compilation requires a complex C toolchain; QEMU-based
# emulation is simpler and reliable for amd64 + arm64.
# ---------------------------------------------------------------
FROM rust:1-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Dependency cache layer: copy manifests first, build a dummy binary,
# then invalidate only when Cargo.toml / Cargo.lock change.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && \
    echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -f target/release/deps/warranty_tracker*

COPY src/ src/
COPY migrations/ migrations/
COPY templates/ templates/
COPY static/ static/
RUN cargo build --release

# ---------------------------------------------------------------
# Stage 2 : Runtime — correct arch image selected by buildx
# ---------------------------------------------------------------
FROM debian:stable-slim

WORKDIR /app

RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 curl && \
    rm -rf /var/lib/apt/lists/* && \
    useradd -r -s /bin/false -u 1001 garantify && \
    mkdir -p /data/uploads && \
    chown garantify:garantify /data/uploads

COPY --from=builder /app/target/release/warranty-tracker ./warranty-tracker
COPY --from=builder /app/migrations/ ./migrations/
COPY --from=builder /app/static/ ./static/

USER garantify

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

LABEL org.opencontainers.image.source="https://github.com/0xNOCARRIER/garantify"
LABEL org.opencontainers.image.description="Self-hosted warranty tracker with email and Slack alerts"
LABEL org.opencontainers.image.licenses="AGPL-3.0-or-later"
LABEL org.opencontainers.image.title="Garantify"

CMD ["./warranty-tracker"]
