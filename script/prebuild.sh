#!/bin/bash
set -euo pipefail

version="$1"
repo="$2"
builddir="$3"
pkg="$4"

build () {
    features="$1"
    cargo clean
    wasm-pack build --target web --release plume-front
    cargo build --release --no-default-features --features="${features}" --package=plume-cli
    cargo build --release --no-default-features --features="${features}"
    ./script/generate_artifact.sh
}

git clone $repo $builddir
cd $builddir
git checkout $version
mkdir -p $pkg
build postgres
mv plume.tar.gz /pkg/plume-postgres.tar.gz
build sqlite
mv plume.tar.gz /pkg/plume-sqlite.tar.gz
