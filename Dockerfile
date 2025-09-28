# Build stage with cache mounts for performance
FROM --platform=$BUILDPLATFORM rust:1.90-slim AS builder

# Enable BuildKit cache mounts
ARG BUILDKIT_INLINE_CACHE=1

ENV USER=appuser
ENV UID=101

# Use a system UID that's within the acceptable range
RUN useradd \
    --system \
    --uid "${UID}" \
    --shell /sbin/nologin \
    --create-home \
    --home-dir /app \
    "${USER}"

WORKDIR /app

# Copy only manifest files first for better caching
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ ./src/

# Build the application (dependencies cached by Docker layer)
RUN cargo build --release

# Final stage - minimal image
FROM debian:bookworm-slim

# Copy essential files
COPY --from=builder /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

# Copy the binary
COPY --from=builder /app/target/release/grumpy /grumpy

ENTRYPOINT ["/grumpy"]
