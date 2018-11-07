#!/bin/bash
wget https://github.com/SimonKagstrom/kcov/archive/master.tar.gz &&
tar xzf master.tar.gz &&
mkdir -p kcov-master/build &&
cd kcov-master/build &&
cmake .. &&
make &&
sudo make install &&
cd ../.. &&
for file in target/debug/*-*[^\.d]; do
	if [[ -x "$file" ]]
	then
		filename=$(basename $file)
		mkdir -p "target/cov/$filename"
		kcov --exclude-pattern=/.cargo,/usr/lib --verify "target/cov/$filename" "$file"
	fi
done &&
bash <(curl -s https://codecov.io/bash) &&
echo "Uploaded code coverage"
