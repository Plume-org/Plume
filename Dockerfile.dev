FROM rust:1-stretch

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
RUN cargo install diesel_cli --no-default-features --features postgres --version '=1.3.0'
RUN cargo install cargo-web

COPY . .

RUN chmod a+x ./script/plume-front.sh && sleep 1 && ./script/plume-front.sh
RUN cargo install --path ./ --force --no-default-features --features postgres
RUN cargo install --path plume-cli --force --no-default-features --features postgres
RUN cargo clean

CMD ["plume"]

EXPOSE 7878
