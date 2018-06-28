# How to install Plume on a Debian stretch:

## Basic setup:
apt update
apt install gettext postgresql postgresql-contrib libpq-dev
adduser plume
su - plume
cd /home/plume
git clone https://github.com/Plume-org/Plume.git
curl https://sh.rustup.rs -sSf | sh -s -- --no-modify-path --default-toolchain nightly
cd Plume
rustup toolchain install nightly
rustup override set nightly-2018-05-15 # this seems to be needed for compilation
cargo install diesel_cli --no-default-features --features postgres # we dont need to compile anything else than pgsql

## Now, if you want to run postgresql on the same server:
cargo run # this will configure and launch Plume on the server.

## If you want to run Plume with a remote DB this time ( Postgresql is not installed on the same server/container):
* On the DB server:
su - postgres
createuser -d -P plume
createdb -O plume plume

* On the Plume server:
diesel migration run --database-url postgres://plume:PASSWORD@DBSERVERIP:DBPORT/plume
DB_URL=postgres://plume:PASSWORD@DBSERVERIP:DBPORT/plume cargo run # the first launch will ask questions to configure the instance. A second launch will not need the DB_URL.
