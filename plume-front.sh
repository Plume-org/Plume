#!/bin/bash

ARCH=`arch`

if [ "$ARCH" == "aarch64" -o "$ARCH" == "armv7l" ] ; then
    PATH_OLD=${PATH}
    export PATH=/opt/local/llvm/bin:${PATH}
    cd /app
    RUSTFLAGS="-C linker=lld" cargo web deploy -p plume-front
    export PATH=${PATH_OLD}
else
    cargo web deploy -p plume-front
fi
