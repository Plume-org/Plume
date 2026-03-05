FROM rust:1.93.1-slim-trixie AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    gettext \
    postgresql-client \
    libpq-dev \
    git \
    curl \
    gcc \
    make \
    openssl \
    pkg-config \
    libssl-dev \
    clang

WORKDIR /scratch
COPY script/wasm-deps.sh .
RUN chmod a+x ./wasm-deps.sh && sleep 1 && ./wasm-deps.sh

WORKDIR /app

COPY . .
RUN cargo install wasm-pack
RUN chmod a+x ./script/plume-front.sh && sleep 1 && ./script/plume-front.sh
RUN cargo build --release --no-default-features --features postgres
RUN cargo build --release --package plume-cli --no-default-features --features postgres

FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libpq5 \
    libssl3t64

WORKDIR /app

COPY --from=builder /app /app
COPY --from=builder /app/target/release/plm /bin/
COPY --from=builder /app/target/release/plume /bin/

CMD ["plume"]

EXPOSE 7878
