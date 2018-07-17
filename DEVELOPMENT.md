# Development Guide

## Running Plume locally

### Mac OSX

All commands are run in the Mac Terminal or terminal emulator of your choice, such as iTerm2. First, you will need [Git](https://git-scm.com/download/mac), [Homebrew](https://brew.sh/), [Rust](https://www.rust-lang.org/en-US/), and [Postgres](https://www.postgresql.org/). Follow the instructions to install Homebrew before continuing if you don't already have it.

### Linux

Similar to Mac OSX all commands should be run from a terminal (a.k.a command line). First, you will need [Git](https://git-scm.com/download/mac), [Rust](https://www.rust-lang.org/en-US/), and [Postgres](https://www.postgresql.org/).  Step-by-step instructions are also available here:  [Installing Prerequisites](/doc/PREREQUISITES.md)

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

When you will launch Plume for the first time, it will setup the database by itself.

#### Database Migration

To run migrations and correctly setup the database, Plume use the `diesel` CLI tool under the hood. Therefore you should install it before running Plume. If this was your time installing Rust, you will probably need to run that using `cargo`. `cargo` is installed with `rustc` so if you followed the earlier instructions it will already be available.

```
cargo install diesel_cli --version '=1.2.0'
```

#### Running Plume

To run Plume locally, make sure you are once again in the Plume directory, such as `~/dev/Plume`. Now you will be able to run the application using the command

```
cargo run
```

#### Configuration

The first time you'll run Plume, it will help you setup your instance through an interactive tool. Once you'll have answered all its question, your instance will start.

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

If you don't want to setup HTTPS locally, you can also disable it by running your instance with `USE_HTTPS=0` set.

```
USE_HTTPS=0 cargo run
```

#### Making a Pull Request
To create an upstream fork of the repository in GitHub, click "Fork" in the top right button on the main page of the [Plume repository](https://github.com/Plume-org/Plume). Now, in the command line, set another remote for the repository by running the following command, replacing `myname` with the name under which you forked the repo. You can use another name besides `upstream` if you prefer. Using [SSH](https://help.github.com/articles/connecting-to-github-with-ssh/) is recommended.

```
git remote add upstream git@github.com/myname/Plume.git
# Alt # git remote add upstream https://github.com/myname/Plume.git
```

Now, make any changes to the code you want. After committing your changes, push to the upstream fork. Once your changes are made, visit the GitHub page for your fork and select "New pull request". Add descriptive text, any issue numbers using hashtags to reference the issue number, screenshots of your changes if relevant, a description of how you tested your changes, and any other information that will help the project maintainers be able to quickly accept your pull requests.

The project maintainers may suggest further changes to improve the pull request even more. After implementing this locally, you can push to your upstream fork again and the changes will immediately show up in the pull request after pushing. Once all the suggested changes are made, the pull request may be accepted. Thanks for contributing.

#### When working with Tera templates

When working with the interface, or any message that will be displayed to the final user, keep in mind that Plume is an internationalized software. To make sure that the parts of the interface you are changing are translatable, you should:

- Use the `_` and `_n` filters instead of directly writing strings in your HTML markup
- Add the strings to translate to the `po/plume.pot` file

Here is an example: let's say we want to add two strings, a simple one and one that may deal with plurals. The first step is to add them to whatever template we want to display them in:

```jinja
<p>{{ "Hello, world!" | _ }}</p>

<p>{{ "You have {{ count }} new notifications" | _n(singular="You have one new notification", count=n_notifications) }}</p>
```

As you can see, the `_` doesn't need any special argument to work, but `_n` requires `singular` (the singular form, in English) and `count` (the number of items, to determine which form to use) to be present. Note that any parameters given to these filters can be used as regular Tera variables inside of the translated strings, like we are doing with the `count` variable in the second string above.

The second step is to add them to POT file. To add a simple message, just do:

```po
msgid "Hello, world" # The string you used with your filter
msgstr "" # Always empty
```

For plural forms, the syntax is a bit different:

```po
msgid "You have one new notification" # The singular form
msgid_plural "You have {{ count }} new notifications" # The plural one
msgstr[0] ""
msgstr[1] ""
```

And that's it! Once these new messages will have been translated, they will correctly be displayed in the requested locale!
