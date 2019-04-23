#!/bin/bash
pr_id=$(basename "$CI_PULL_REQUEST")
[ -z "$pr_id" ] && exit
backend="$FEATURES"
password="$JOINPLUME_PASSWORD"

curl -T plume.tar.gz "https://circleci:$password@joinplu.me/upload_pr/$backend/$pr_id.tar.gz"
