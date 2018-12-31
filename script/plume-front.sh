#!/bin/bash

ARCH=`arch`

if [ "$ARCH" == "aarch64" -o "$ARCH" == "armv7l" ] ; then
    export PATH=/opt/local/llvm/bin:${PATH}
    cd /app
    RUSTFLAGS="-C linker=lld" cargo web deploy -p plume-front
else
    cargo web deploy -p plume-front
fi
