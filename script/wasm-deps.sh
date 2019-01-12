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

echo "Detected arch: $ARCH"

if [ $ARCH == "aarch64" -o $ARCH == "armv71" ] ; then
    apt-get install -y --no-install-recommends build-essential subversion ninja-build cmake
    mkdir -p /scratch/src
    cd /scratch/src
    # Pin LLVM to post 7.0.1 tag and pin to a known-good revision for Plume builds
    svn co -r350977 http://llvm.org/svn/llvm-project/llvm/trunk/ llvm
    cd /scratch/src/llvm/tools
    # Pin lld to post 7.0.1 tag and pin to a known-good revision for Plume builds
    svn co -r350975 http://llvm.org/svn/llvm-project/lld/trunk lld
    #svn co http://llvm.org/svn/llvm-project/cfe/trunk clang
    #svn co http://llvm.org/svn/llvm-project/clang-tools-extra/trunk extra
    mkdir -p /scratch/build/arm
    cd /scratch/build/arm
    if [ "$ARCH" == "aarch64" ] ; then
        cmake -G Ninja /scratch/src/llvm \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=/opt/local/llvm \
            -DLLVM_TARGETS_TO_BUILD="AArch64" \
            -DLLVM_TARGET_ARCH="AArch64"
    else
        cmake -G Ninja /scratch/src/llvm \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=/opt/local/llvm \
            -DLLVM_TARGETS_TO_BUILD="ARM" \
            -DLLVM_TARGET_ARCH="ARM"
    fi
    ninja lld
    ninja install-lld
    cd ~
    rm -rf /scratch
fi
