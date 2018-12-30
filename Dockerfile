FROM rust:1-stretch

RUN apt-get update && apt-get install -y --no-install-recommends \
    gettext \
    postgresql-client \
    libpq-dev \
    git \
    curl \
    gcc \
    make \
    openssl \
    libssl-dev
WORKDIR /app
RUN chmod a+x ./wasm-deps.sh && ./wasm-deps.sh
COPY Cargo.toml Cargo.lock ./
RUN cargo install diesel_cli --no-default-features --features postgres --version '=1.3.0'
RUN cargo install cargo-web
COPY . .
RUN chmod a+x ./plume-front.sh && ./plume-front.sh
RUN cargo install --path ./ --force --no-default-features --features postgres
RUN cargo install --path plume-cli --force --no-default-features --features postgres
RUN cargo clean
CMD ["plume"]
EXPOSE 7878
