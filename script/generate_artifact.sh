#!/bin/bash
mkdir bin
cp target/release/{plume,plm} bin
strip -s bin/*
tar -cvzf plume.tar.gz bin/ static/
tar -cvzf wasm.tar.gz static/plume_front{.js,_bg.wasm}
