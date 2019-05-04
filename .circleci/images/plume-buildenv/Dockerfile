FROM debian:stretch-20190326
ENV PATH="/root/.cargo/bin:${PATH}"

#install native/circleci/build dependancies
RUN apt update &&\
    apt install -y --no-install-recommends git ssh tar gzip ca-certificates default-jre&&\
    apt install -y --no-install-recommends binutils-dev build-essential cmake curl gcc gettext git libcurl4-openssl-dev libdw-dev libelf-dev libiberty-dev libpq-dev libsqlite3-dev libssl-dev make openssl pkg-config postgresql postgresql-contrib python zlib1g-dev python3-pip zip unzip &&\
    rm -rf /var/lib/apt/lists/*

#install and configure rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly-2019-03-23 -y &&\
    rustup component add rustfmt clippy &&\
    rustup component add rust-std --target wasm32-unknown-unknown

#compile some deps
RUN cargo install cargo-web &&\
    cargo install grcov &&\
    strip /root/.cargo/bin/* &&\
    rm -fr ~/.cargo/registry

#set some compilation parametters
COPY cargo_config /root/.cargo/config

#install selenium for front end tests
RUN pip3 install selenium

#install and configure caddy
RUN curl https://getcaddy.com | bash -s personal
COPY Caddyfile /Caddyfile

#install crowdin
RUN mkdir /crowdin && cd /crowdin &&\
    curl -O https://downloads.crowdin.com/cli/v2/crowdin-cli.zip &&\
    unzip crowdin-cli.zip && rm crowdin-cli.zip &&\
    cd * && mv crowdin-cli.jar /usr/local/bin && cd && rm -rf /crowdin &&\
    /bin/echo -e '#!/bin/sh\njava -jar /usr/local/bin/crowdin-cli.jar $@' > /usr/local/bin/crowdin &&\
    chmod +x /usr/local/bin/crowdin
