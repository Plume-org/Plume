#!/bin/bash 
set -eo pipefail
for file in target/debug/*-*[^\.d]; do
	if [[ -x "$file" ]]
	then
		filename=$(basename $file)
		mkdir -p "target/cov/$filename"
		kcov --exclude-pattern=/.cargo,/usr/lib --verify "target/cov/$filename" "$file"
	fi
done
bash <(curl -s https://codecov.io/bash)
