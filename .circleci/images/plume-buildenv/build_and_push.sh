#!/bin/bash
[ "$1" = "" ] && echo "you must provide one argument, the build version" && exit 1
docker build -t plumeorg/plume-buildenv:$1 .
docker push plumeorg/plume-buildenv:$1
