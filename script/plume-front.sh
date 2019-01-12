#!/bin/bash

ARCH=$(python <<EOF
from __future__ import print_function
import platform
processor = platform.machine()
architecture = platform.architecture()
if processor == 'aarch64':
    # Mutli arch arm support is why this 32bit check is present
    if '32bit' in architecture:
        print('armv71', end='')
    else:
        print('aarch64', end='')
elif processor == 'x86 64' or processor == 'x86_64':
    print('amd64', end='')
elif processor == 'armv7l':
    print('armhf', end='')
EOF
)

if [ "$ARCH" == "aarch64" -o "$ARCH" == "armv7l" ] ; then
    export PATH=/opt/local/llvm/bin:${PATH}
    cd /app
    RUSTFLAGS="-C linker=lld" cargo web deploy -p plume-front
else
    cargo web deploy -p plume-front
fi
