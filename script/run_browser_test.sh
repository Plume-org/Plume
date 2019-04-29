#!/bin/bash
set -eo pipefail

export ROCKET_SECRET_KEY="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="

mkdir -p "target/cov/plume"
mkdir -p "target/cov/plm"
plm='kcov --exclude-pattern=/.cargo,/usr/lib --verify target/cov/plm plm'

$plm migration run
$plm migration redo
$plm instance new -d plume-test.local -n plume-test
$plm users new -n admin -N 'Admin' -e 'email@exemple.com' -p 'password'

kcov --exclude-pattern=/.cargo,/usr/lib --verify target/cov/plume plume &
caddy -conf /Caddyfile &

until curl http://localhost:7878/test/health -f; do sleep 1; done 2>/dev/null >/dev/null

cd $(dirname $0)/browser_test/
python3 -m unittest *.py

kill -SIGINT  %1
kill -SIGKILL %2
sleep 5
