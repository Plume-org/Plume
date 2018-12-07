# Development Guide

## Installing the development environment

Please refer to the [installation guide](INSTALL.md).

## Testing the federation

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

## Making a Pull Request
To create an upstream fork of the repository in GitHub, click "Fork" in the top right button on the main page of the [Plume repository](https://github.com/Plume-org/Plume). Now, in the command line, set another remote for the repository by running the following command, replacing `myname` with the name under which you forked the repo. You can use another name besides `upstream` if you prefer. Using [SSH](https://help.github.com/articles/connecting-to-github-with-ssh/) is recommended.

```
git remote add upstream git@github.com/myname/Plume.git
# Alt # git remote add upstream https://github.com/myname/Plume.git
```

Now, make any changes to the code you want. After committing your changes, push to the upstream fork. Once your changes are made, visit the GitHub page for your fork and select "New pull request". Add descriptive text, any issue numbers using hashtags to reference the issue number, screenshots of your changes if relevant, a description of how you tested your changes, and any other information that will help the project maintainers be able to quickly accept your pull requests.

The project maintainers may suggest further changes to improve the pull request even more. After implementing this locally, you can push to your upstream fork again and the changes will immediately show up in the pull request after pushing. Once all the suggested changes are made, the pull request may be accepted. Thanks for contributing.

## When working with Ructe templates

When working with the interface, or any message that will be displayed to the final user,
keep in mind that Plume is an internationalized software.
To make sure that the parts of the interface you are changing are translatable, you should:

- Wrap strings to translate in the `i18n!` macro (see [rocket_i18n docs](https://docs.rs/rocket_i18n/)
for more details about its arguments).The `Catalog` argument is usually `ctx.1`.
- Add the strings to translate to the `po/plume.pot` file

Here is an example: let's say we want to add two strings, a simple one and one
that may deal with plurals. The first step is to add them to whatever
template we want to display them in:

```html
<p>@i18n!(ctx.1, "Hello, world!")</p>

<p>@i18n!(ctx.1, "You have one new notification", "You have {0} new notifications", n_notifications)</p>
```

The second step is to add them to POT file. To add a simple message, just do:

```po
msgid "Hello, world" # The string you used with your filter
msgstr "" # Always empty
```

For plural forms, the syntax is a bit different:

```po
msgid "You have one new notification" # The singular form
msgid_plural "You have {0} new notifications" # The plural one
msgstr[0] ""
msgstr[1] ""
```

And that's it! Once these new messages will have been translated, they will correctly be displayed in the requested locale!

## Code Style

For Rust, use the standard style. `rustfmt` can help you keeping your code clean.

For CSS, the only rule is to use One True Brace Style.

For JavaScript, we use [the JavaScript Standard Style](https://standardjs.com/).

For HTML/Ructe templates, we use HTML5 syntax.
