FROM rust:1-buster
ENV PATH="/root/.cargo/bin:${PATH}"

#install native/circleci/build dependancies
RUN apt update &&\
    apt install -y --no-install-recommends git ssh tar gzip ca-certificates default-jre&&\
    echo "deb [trusted=yes] https://apt.fury.io/caddy/ /" \
    | tee -a /etc/apt/sources.list.d/caddy-fury.list &&\
    wget -qO - https://artifacts.crowdin.com/repo/GPG-KEY-crowdin | apt-key add - &&\
    echo "deb https://artifacts.crowdin.com/repo/deb/ /" > /etc/apt/sources.list.d/crowdin.list &&\
    apt update &&\
    apt install -y --no-install-recommends binutils-dev build-essential cmake curl gcc gettext git libcurl4-openssl-dev libdw-dev libelf-dev libiberty-dev libpq-dev libsqlite3-dev libssl-dev make openssl pkg-config postgresql postgresql-contrib python zlib1g-dev python3-dev python3-pip python3-setuptools zip unzip libclang-dev clang caddy crowdin3 &&\
    rm -rf /var/lib/apt/lists/*

#stick rust environment
COPY rust-toolchain ./

#compile some deps
RUN cargo install wasm-pack &&\
    cargo install grcov &&\
    rm -fr ~/.cargo/registry

#set some compilation parametters
COPY cargo_config /root/.cargo/config

#install selenium for front end tests
RUN pip3 install selenium

#configure caddy
COPY Caddyfile /Caddyfile
