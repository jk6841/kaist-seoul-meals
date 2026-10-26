FROM rust:1.84-bookworm AS builder

WORKDIR /home

# 의존성만 먼저 빌드해 별도 레이어로 캐시한다.
# Cargo.toml / Cargo.lock이 바뀌지 않으면 이 레이어를 재사용한다.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release --locked && \
    rm -rf src

COPY src ./src
# 더미 main.rs로 빌드된 결과를 확실히 다시 빌드하도록 수정 시각 갱신
RUN touch src/main.rs && \
    cargo build --release --locked --bin kaist-seoul-meals && \
    strip --strip-all target/release/kaist-seoul-meals

# glibc에 동적 링크된 바이너리이므로 libc가 포함된 distroless/cc 사용.
# TLS는 rustls + 내장 루트 인증서를 쓰므로 OpenSSL이나 CA 인증서 파일은 필요 없다.
FROM gcr.io/distroless/cc-debian12

WORKDIR /home

COPY --from=builder /home/target/release/kaist-seoul-meals ./app

CMD ["/home/app"]
