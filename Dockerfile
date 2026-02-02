FROM rust:1.84-bookworm AS builder

WORKDIR /home

COPY Cargo.toml Cargo.lock* ./

COPY . .
RUN cargo build --release --bin kaist-seoul-meals && \
    strip --strip-all target/release/kaist-seoul-meals

FROM gcr.io/distroless/cc-debian12

WORKDIR /home

# 정적 바이너리만 복사
COPY --from=builder /home/target/release/kaist-seoul-meals ./app

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

EXPOSE 3000

CMD ["/home/app"]