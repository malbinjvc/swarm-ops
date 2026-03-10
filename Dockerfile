# ── Builder stage ─────────────────────────────────────────────────
FROM rust:1.83-alpine AS builder
RUN apk add --no-cache musl-tools
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
RUN cargo build --release

# ── Runtime stage ────────────────────────────────────────────────
FROM alpine:3.20
RUN addgroup -S appgroup && adduser -S appuser -G appgroup
COPY --from=builder /app/target/release/swarm-ops /usr/local/bin/swarm-ops
USER appuser
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD wget -qO- http://localhost:3000/health || exit 1
CMD ["swarm-ops"]
