#!/bin/bash

ARCH=`arch`

if [ "$ARCH" == "aarch64" -o "$ARCH" == "armv7l" ] ; then
    apt-get install -y --no-install-recommends build-essential subversion ninja-build cmake
    mkdir -p /scratch/src
    cd /scratch/src
    svn co http://llvm.org/svn/llvm-project/llvm/trunk llvm
    cd /scratch/src/llvm/tools
    svn co http://llvm.org/svn/llvm-project/lld/trunk lld
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
    export PATH_OLD=${PATH}
    export PATH=/opt/local/llvm/bin:${PATH}
    cd /app
    RUSTFLAGS="-C linker=lld" cargo web deploy -p plume-front
    rm -rf /opt/*
    export PATH=${PATH_OLD}
else
    cargo web deploy -p plume-front
fi
