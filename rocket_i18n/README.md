# Rocket I18N

A crate to help you internationalize your Rocket or Actix Web applications.

It just selects the correct locale for each request, and return the corresponding `gettext::Catalog`.

## Usage

First add it to your `Cargo.toml`:

```toml
[dependencies]
rocket_i18n = "0.5.1"
gettext-macros = "0.6.1" # Provides proc-macros to manage translations
```

Then, in your `main.rs`:

```rust,ignore
# use rocket;
use gettext_macros::{compile_i18n, include_i18n, init_i18n};

init_i18n!("my_web_app", en, eo, it, pl);

fn main() {
    rocket::ignite()
        // Make Rocket manage your translations.
        .manage(include_i18n!());
        // Register routes, etc
}

compile_i18n!();
```

Then in all your requests you'll be able to use the `i18n` macro to translate anything.
It takes a `gettext::Catalog` and a string to translate as argument.

```rust,ignore
use gettext_macros::i18n;
use rocket_i18n::I18n;

#[get("/")]
fn route(i18n: I18n) -> &str {
    i18n!(i18n.catalog, "Hello, world!")
}
```

For strings that may have a plural form, just add the plural and the number of element to the
arguments

```rust,ignore
i18n!(i18n.catalog, "One new message", "{0} new messages", 42);
```

Any extra argument, after a `;`, will be used for formatting.

```rust,ignore
let user_name = "Alex";
i18n!(i18n.catalog, "Hello {0}!"; user_name);
```

When using it with plural, `{0}` will be the number of elements, and other arguments will start
at `{1}`.

Because of its design, rocket_i18n is only compatible with askama, ructe or compiled templates
in general.
You can use the `t` macro in your templates, as long as they have a field called `catalog` to
store your catalog.

### Using with Actix Web

First, disable the default features so it doesn't pull in all of Rocket.

```toml
[dependencies.rocket_i18n]
version = "0.4"
default-features = false
features = ["actix-web"]
```

Then add it to your application.

```rust
use gettext_macros::*;
use rocket_i18n::{I18n, Internationalized, Translations};

fn route_handler(i18n: I18n) -> &str {
    i18n!(i18n.catalog, "Hello, world!")
}

#[derive(Clone)]
struct MyState {
    translations: Translations,
}

impl Internationalized for MyState {
    fn get(&self) -> Translations {
        self.translations.clone()
    }
}

fn main() {
    let state = MyState {
        translations: rocket_i18n::i18n("your-domain", vec![ "en", "fr", "de", "ja" ]);
    };

    App::with_state(state)
        .resource("", |r| r.with(route_handler))
        .finish();
}
```
