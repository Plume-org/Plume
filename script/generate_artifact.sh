#!/bin/bash
mkdir bin
cp target/release/{plume,plm} bin
tar -cvzf plume.tar.gz bin/ static/
