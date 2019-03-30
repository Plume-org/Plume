FROM debian:stretch-20190326
ENV PATH="/root/.cargo/bin:${PATH}"

#install native/circleci/build dependancies
RUN apt update &&\
    apt install -y git ssh tar gzip ca-certificates &&\
    apt install -y binutils-dev build-essential cmake curl gcc gettext git libcurl4-openssl-dev libdw-dev libelf-dev libiberty-dev libpq-dev libsqlite3-dev libssl-dev make openssl pkg-config postgresql postgresql-contrib python zlib1g-dev

#install and configure rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly-2019-03-23 -y &&\
    rustup component add rustfmt clippy &&\
    rustup component add rust-std --target wasm32-unknown-unknown &&\
    cargo install cargo-web &&\
    rm -fr ~/.cargo/registry

#install coverage tools
RUN curl -L https://github.com/SimonKagstrom/kcov/archive/master.tar.gz | tar xz &&\
    mkdir -p kcov-master/build && cd kcov-master/build && cmake .. && make &&\
    make install && cd ../.. && rm -rf kcov-master
