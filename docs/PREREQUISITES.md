# Installing Software Prerequisites

These instructions have been adapted from the Aardwolf documentation, and may not be accurate.
As such, this notification should be updated once verified for Plume installs.

> NOTE: These instructions may help in installing a production version, but are
intended for developers to be able to build and test their changes. If in doubt,
seek out documentation from your distribution package or from [the `docs` folder](docs).

## Installing Requirements

### Installing PostgreSQL

In order to run the Plume backend, you will need to have access to a
[PostgreSQL](https://www.postgresql.org/) database. There are a few options for doing this, but for
this guide we’re going to assume you are running the database on your
development machine.

#### Linux/OSX Instructions

If you're on an Ubuntu-like machine, you should be able to install
PostgreSQL like this:

    $ sudo apt-get update
    $ sudo apt-get install postgresql postgresql-contrib

If you see an error like:

     = note: /usr/bin/ld: cannot find -lpq
          collect2: error: ld returned 1 exit statusb

Then you may need to install the libpq (PostgreSQL C-library) package as well :

    $ sudo apt-get install libpq-dev

If you're on OSX and using `brew`, do

    $ brew update
    $ brew install postgres

For Gentoo (eselect-postgresql is optional),

    # emerge --sync
    # emerge -av postgresql eselect-postgresql

For Fedora/CentOS/RHEL, do

    # dnf install postgresql-server postgresql-contrib mariadb-devel libsq3-devel libpqxx libpqxx-devel


#### Windows Instructions

For Windows, just download the installer [here](https://www.enterprisedb.com/downloads/postgres-postgresql-downloads#windows) and run it. After installing, make sure to add the <POSTGRES INSTALL PATH>/lib directory to your PATH system variable.

### Installing rustup

> Note: Rustup managed installations do appear to co-exist with system
 installations on Gentoo, and should work on most other distributions.
 If not, please file an issue with the Rust and Rustup teams or your distribution’s
 managers.

Next, you’ll need to have the [Rust](https://rust-lang.org/) toolchain
installed. The best way to do this is to install
[rustup](https://rustup.rs), which is a Rust toolchain manager.

#### Linux/OSX Instructions

Open your terminal and run the following command:

    $ curl https://sh.rustup.rs -sSf | sh

For those who are (understandably) uncomfortable with piping a shell
script from the internet directly into `sh`, you can also
[use an alternate installation method](https://github.com/rust-lang-nursery/rustup.rs/#other-installation-methods).

#### Windows Instructions

If you don't already have them, download and install the [Visual C++ 2015 Build Tools](http://landinghub.visualstudio.com/visual-cpp-build-tools).

Then, download the [rustup installer](https://www.rust-lang.org/en-US/install.html) and run it. That's it!

### Installing Rust Toolchain

Once you have `rustup` installed, make sure you have the `nightly` rust
toolchain installed:

    $ rustup toolchain install nightly

### Installing GetText

GetText is the tool we use to manage translations. It will be needed at runtime, since we only compile
translation files when starting Plume.

#### Ubuntu-like Linux

    $ sudo apt-get install gettext
