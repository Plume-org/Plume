#!/bin/bash
set -euo pipefail

version="$1"

docker run --rm -v $PWD:/repo -v $PWD/pkg:/pkg -v $PWD/script/prebuild.sh:/prebuild.sh plumeorg/plume-buildenv:v0.4.0 /prebuild.sh "$version" /repo /prebuild /pkg
