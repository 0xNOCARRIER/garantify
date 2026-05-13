# ---------------------------------------------------------------
# Stage 1 : Compilation
# ---------------------------------------------------------------
FROM rust:1-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Cache des dépendances : on copie les manifestes seuls et on compile
# un binaire vide. Docker met ce layer en cache tant que Cargo.toml
# et Cargo.lock ne changent pas.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && \
    echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -f target/release/deps/warranty_tracker*

# Copie du vrai code source
COPY src/ src/
COPY migrations/ migrations/
COPY templates/ templates/
COPY static/ static/
RUN cargo build --release

# ---------------------------------------------------------------
# Stage 2 : Image finale légère
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

CMD ["./warranty-tracker"]
