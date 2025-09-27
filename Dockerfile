FROM rust:1.90-slim AS builder

ENV USER=appuser
ENV UID=10001

RUN useradd \
    --system \
    --uid "${UID}" \
    --shell /sbin/nologin \
    --create-home \
    --home-dir /app \
    "${USER}"

WORKDIR /app

COPY Cargo.toml ./
COPY src/ ./src/
RUN cargo build --release

FROM scratch

COPY --from=builder /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

COPY --from=builder /app/target/release/grumpy /bin/grumpy

USER appuser:appuser

ENTRYPOINT ["/bin/grumpy"]
