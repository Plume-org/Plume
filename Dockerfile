FROM rust:1-stretch as builder

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
    libssl-dev

WORKDIR /scratch
COPY script/wasm-deps.sh .
RUN chmod a+x ./wasm-deps.sh && sleep 1 && ./wasm-deps.sh

WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain ./
RUN cargo install cargo-web

COPY . .

RUN chmod a+x ./script/plume-front.sh && sleep 1 && ./script/plume-front.sh
RUN cargo install --path ./ --force --no-default-features --features postgres
RUN cargo install --path plume-cli --force --no-default-features --features postgres
RUN cargo clean

FROM debian:stretch-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libpq5 \
    libssl1.1

WORKDIR /app

COPY --from=builder /app /app
COPY --from=builder /usr/local/cargo/bin/plm /bin/
COPY --from=builder /usr/local/cargo/bin/plume /bin/

CMD ["plume"]

EXPOSE 7878
