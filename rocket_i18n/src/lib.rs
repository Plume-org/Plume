//! # Rocket I18N
//!
//! A crate to help you internationalize your Rocket or Actix Web applications.
//!
//! It just selects the correct locale for each request, and return the corresponding `gettext::Catalog`.
//!
//! ## Usage
//!
//! First add it to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! rocket_i18n = "0.5.1"
//! gettext-macros = "0.1" # Provides proc-macros to manage translations
//! ```
//!
//! Then, in your `main.rs`, add the translations to you application's data:
//!
//! ```rust,ignore
//! App::new()
//!     .data(rocket_i18n::i18n("your-domain", vec!["en", "pl"]))
//!     .service(...)
//! ```
//!
//!
//! Then in all your requests you'll be able to use the `i18n` macro to translate anything.
//! It takes a `gettext::Catalog` and a string to translate as argument.
//!
//! ```rust,ignore
//! fn route(i18n: I18n) -> String {
//!     i18n!(i18n.catalog, "Hello, world!")
//! }
//! ```
//!
//! For strings that may have a plural form, just add the plural and the number of element to the
//! arguments
//!
//! ```rust,ignore
//! i18n!(i18n.catalog, "One new message", "{0} new messages", 42);
//! ```
//!
//! Any extra argument, after a `;`, will be used for formatting.
//!
//! ```rust,ignore
//! let user_name = "Alex";
//! i18n!(i18n.catalog, "Hello {0}!"; user_name);
//! ```
//!
//! When using it with plural, `{0}` will be the number of elements, and other arguments will start
//! at `{1}`.
//!
//! Because of its design, rocket_i18n is only compatible with askama, ructe or compiled templates
//! in general.
//! You can use the `t` macro in your templates, as long as they have a field called `catalog` to
//! store your catalog.

pub use gettext::*;
use std::fs;

mod with_rocket;

const ACCEPT_LANG: &str = "Accept-Language";

/// A request guard to get the right translation catalog for the current request.
pub struct I18n {
    /// The catalog containing the translated messages, in the correct locale for this request.
    pub catalog: Catalog,
    /// The language of the current request.
    pub lang: &'static str,
}

pub type Translations = Vec<(&'static str, Catalog)>;

/// Loads translations at runtime. Usually used with `actix_web::web::App::data`.
///
/// Note that the `.mo` files should be present with your binary. If you want to embed them,
/// use `gettext_macros::include_i18n`.
pub fn i18n(domain: &str, lang: Vec<&'static str>) -> Translations {
    lang.iter().fold(Vec::new(), |mut trans, l| {
        let mo_file = fs::File::open(format!("translations/{}/LC_MESSAGES/{}.mo", l, domain))
            .expect("Couldn't open catalog");
        let cat = Catalog::parse(mo_file)
            .unwrap_or_else(|_| panic!("Error while loading catalog ({})", l));
        trans.push((l, cat));
        trans
    })
}

/// Works the same way as `gettext_macros::i18n`, but without needing to give a `gettext::Catalog`
/// as first argument.
///
/// For use in askama templates.
#[macro_export]
macro_rules! t {
    ($( $args:tt )+) => {
        i18n!(self.catalog, $( $args )+)
    };
}
