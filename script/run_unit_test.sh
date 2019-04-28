#!/bin/bash 
set -eo pipefail
for file in target/debug/*-*[^\.d]; do
	if [[ -x "$file" ]]
	then
		filename=$(basename $file)
		if [[ "plume_macro-c53c304b7b109570" =~ ^plume_macro ]]; then
			rm $file
			continue
		fi
		mkdir -p "target/cov/$filename"
		kcov --exclude-pattern=/.cargo,/usr/lib --verify "target/cov/$filename" "$file"
		rm $file
	fi
done
