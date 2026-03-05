# Gettext macros [![Crates.io](https://img.shields.io/crates/v/gettext-macros.svg)](https://crates.io/crates/gettext-macros) [![Docs.rs](https://docs.rs/gettext-macros/badge.svg)](https://docs.rs/gettext-macros)

A few proc-macros to help you internationalize your Rust apps.

It uses gettext under the hood. The idea is that just by wrapping strings in macros, they are added to
a template for translation files (`.pot` file). Then, the actual translation files (`.po`) are generated
for each language, and you can upload them to Weblate/Crowdin/POedit/whatever to have them translated.
And finally, they get transformed into binary translation files (`.mo`) that you can embed in your app.

## How does it works?

There are five main macros:

- `init_i18n`, that should be called first. It tells the domain to use for the current
crate, and the supported locales.
- `compile_i18n`, that should be called at the end of your `main.rs`. It updates translation files and compile them.
- `include_i18n`, that will embed translations in your binary, making it easier to distribute. It should be called after `compile_i18n` to work correctly.
- `i18n`, that translates a given message.
- `t`, that works like `i18n`, but doesn't actually translate the message, just adds it to the list of strings to translate.

The advantage of these macros is that they allow you to work with multiple translation
domains (for instance, one for each of your workspace's crate), and that they automatically
generate a .pot file for these domains.

## Example

*main.rs*

```rust,ignore
use gettext_macros::*;

// The translations for this crate are stored in the "my_app" domain.
// Translations for all the listed languages will be available.
init_i18n!("my_app", ar, de, en, fr, it, ja, ru);

fn main() {
    let catalog = cat();

    println!("{}", i18n!(catalog, "Hello, world!"));
    let name = "Jane";
    println!("{}", i18n!(catalog, "Hello, {}!"; name));
    let message_count = 42;
    println!("{}", i18n!(catalog, "You have one new message", "You have {0} new messages"; message_count));
}

fn cat() -> gettext::Catalog {
    // include_i18n! embeds translations in your binary.
    // It gives a Vec<(&'static str, Catalog)> (list of catalogs with their associated language).
    let catalog = include_i18n!()[0]
}

// Generate or update .po from .pot, and compile them to .mo
compile_i18n!();
```

## Order of the macros

The macros should be called in a certain order to work properly. This order
doesn't depend on the program flow, but of the Rust parser flow. Rust will execute
macros in the same order they are written in your code. For instance:

```rust,ignore
i_am_expanded_first!();
then_i_am_expanded!()
```

Or, for projects with modules:

```rust,ignore
// In main.rs

first_macro!();

mod a;

third_macro!();
```

```rust,ignore
// In a.rs

second_macro!();
```

So, for the macros provided by this crate, the order to follow is:

1. `init_i18n!`
2. `i18n!` and `t!`, as many times as you want
3. `compile_i18n!`
4. `include_i18n!`

Because some of these macros depends on files written by the previous ones to work properly.
