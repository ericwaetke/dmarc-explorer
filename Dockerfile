FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev && \
    rustup target add x86_64-unknown-linux-musl

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo "fn main() {}" > src/main.rs

RUN cargo build --target x86_64-unknown-linux-musl --release

COPY src ./src
COPY templates ./templates
COPY migrations ./migrations

RUN touch src/main.rs && cargo build --target x86_64-unknown-linux-musl --release

FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata && mkdir -p /data

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/dmarc-explorer /usr/local/bin/dmarc-explorer

EXPOSE 3000

ENTRYPOINT ["dmarc-explorer"]
