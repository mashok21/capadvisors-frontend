FROM rust:1.88-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin capadvisors-backend

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates qpdf \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin
COPY --from=builder /app/target/release/capadvisors-backend ./capadvisors-backend

CMD ["./capadvisors-backend"]
