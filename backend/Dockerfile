FROM rust:1.75-alpine AS builder

RUN apk add --no-cache musl-dev perl make

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.19

RUN apk add --no-cache ca-certificates tzdata

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/cnc-spindle-monitor /app/
COPY config.toml /app/

ENV TZ=Asia/Shanghai

EXPOSE 8080 8081 9090

USER 1000

CMD ["./cnc-spindle-monitor"]
