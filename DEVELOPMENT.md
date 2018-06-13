# Development Guide
## Running Plume locally
### Mac OSX
All commands are run in the Mac Terminal or terminal emulator of your choice, such as iTerm2. First, you will need [Git](https://git-scm.com/download/mac), [Homebrew](https://brew.sh/), [Rust](https://www.rust-lang.org/en-US/), and [Postgres](https://www.postgresql.org/). Follow the instructions to install Homebrew before continuing if you don't already have it.
#### Download the Repository
Navigate to the directory on your machine where you would like to install the repository, such as in `~/dev` by running `cd dev`. Now, clone the remote repository by running `git clone https://github.com/Plume-org/Plume.git`. This will install the codebase to the `Plume` subdirectory. Navigate into that directory by running `cd Plume`.
#### Rust
If you think you might already have rust on your machine, you can check by running 
```
rustc --version
# Should output something like
# rustc 1.28.0-nightly (a805a2a5e 2018-06-10)
```
If you don't already have Rust, install it by running
```
curl https://sh.rustup.rs -sSf | sh
```
In the interactive installation, choose the option of the nightly toolchain. Restart your console so that the `rustc` CLI tool is available.
#### Postgres
Now we will use Homebrew to install Postgres. If you think you might already have it, try running `brew info postgres`. If it is not available, continue to install Postgres by running the following:
```
brew install postgres
```
Now, you can use the following command to start Postgres on a one-time basis. 
```
pg_ctl -D /usr/local/var/postgres start
```
After starting Postgres, we need to enter [PSQL](http://postgresguide.com/utilities/psql.html), the interactive terminal for running postgres queries. We'll be running this as the user `postgres` which is an admin-type postgres user.
```
psql postgres
```
Now that you are in psql, enter the following queries to prepare the database for Plume.
```
CREATE DATABASE plume;
CREATE USER plume WITH PASSWORD 'plume';
GRANT ALL PRIVILEGES ON DATABASE plume to plume;
\q
```
The final command `\q` lets us exit psql and returns us to the Terminal. Now, we will open psql again, this time as the `plume` user we just created. Then we'll give all privileges on all tables and sequences to our `plume` user. This is for local development use only and it's not recommend to give complete access to this user in a production environment.
```
psql plume
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO plume;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO plume;
\q
```
#### Database Migration
Now that the Postgres database is set up and the `plume` user has the privileges it  needs, we can set up the database using the diesel CLI. If this was your time installing Rust, you will probably need to run that using `cargo`. `cargo` is installed with `rustc` so if you followed the earlier instructions it will already be available.
```
cargo install diesel_cli
```
The first time you run this, you can run setup. After that, every time you pull the repository you will want to run the migration command in case there were any migrations. Those commands are
```
diesel setup --database-url='postgres://localhost/plume'
diesel migration run --database-url='postgres://localhost/plume'
```
#### Running Plume
To run Plume locally, make sure you are once again in the Plume directory, such as `~/dev/Plume`. Now you will be able to run the application using the command
```
cargo run
```
#### Configuration
Now Plume should be running on your machine at [http://localhost:8000](http://localhost:8000). The first time you run the application, you'll want to configure your blog name on the [http://localhost:8000/configuration](http://localhost:8000/configuration) page. You'll be able to change this name later.
#### Testing the federation

To test the federation, you'll need to setup another database (see "Setup the database"),
also owned by the "plume" user, but with a different name. Then, you'll need to run the
migrations for this database too.

```
diesel migration run --database-url postgres://plume:plume@localhost/my_other_plume_db
```

To run this other instance, you'll need to give two environment variables:

- `ROCKET_PORT`, the port on which your app will run
- `DB_NAME`, the name of the database you just created

```
ROCKET_PORT=3033 DB_NAME=my_other_plume_db cargo run
```
#### Making a Pull Request
To create an upstream fork of the repository in GitHub, click "Fork" in the top right button on the main page of the [Plume repository](https://github.com/Plume-org/Plume). Now, in the command line, set another remote for the repository by running the following command, replacing `myname` with the name under which you forked the repo. You can use another name besides `upstream` if you prefer. Using [SSH](https://help.github.com/articles/connecting-to-github-with-ssh/) is recommended.
```
git remote add upstream git@github.com/myname/Plume.git
# Alt # git remote add upstream https://github.com/myname/Plume.git
```
Now, make any changes to the code you want. After committing your changes, push to the upstream fork. Once your changes are made, visit the GitHub page for your fork and select "New pull request". Add descriptive text, any issue numbers using hashtags to reference the issue number, screenshots of your changes if relevant, a description of how you tested your changes, and any other information that will help the project maintainers be able to quickly accept your pull requests.

The project maintainers may suggest further changes to improve the pull request even more. After implementing this locally, you can push to your upstream fork again and the changes will immediately show up in the pull request after pushing. Once all the suggested changes are made, the pull request may be accepted. Thanks for contributing.
