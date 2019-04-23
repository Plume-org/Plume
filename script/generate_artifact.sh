#!/bin/bash
mkdir bin
cp target/release/{plume,plm} bin
cp "$(which diesel)" bin
strip -s bin/*
tar -cvzf plume.tar.gz bin/ static/ migrations/$FEATURES
