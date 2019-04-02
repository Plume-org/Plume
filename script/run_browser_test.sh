#!/bin/bash
set -eo pipefail

export ROCKET_SECRET_KEY="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="

mkdir -p "target/cov/plume"
mkdir -p "target/cov/plm"
plm='kcov --exclude-pattern=/.cargo,/usr/lib --verify target/cov/plm plm'

$plm instance new -d plume-test.local -n plume-test
$plm users new -n admin -N 'Admin' -e 'email@exemple.com' -p 'password'
$plm search init

kcov --exclude-pattern=/.cargo,/usr/lib --verify target/cov/plume plume &

until curl http://localhost:8000/test/health -f; do sleep 1; done

python3 script/run_browser_test.py

kill -SIGINT %1
wait
