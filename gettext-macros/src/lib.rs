//! A set of macros to make i18n easier.

extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::{
    token_stream::IntoIter as TokenIter, Literal, TokenTree,
};
use quote::quote;
use std::{
    env,
    fs::{create_dir_all, read, File, OpenOptions},
    io::{BufRead, Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, Stdio},
};
use syn::Token;

fn is(t: &TokenTree, ch: char) -> bool {
    match t {
        TokenTree::Punct(p) => p.as_char() == ch,
        _ => false,
    }
}

fn named_arg(mut input: TokenIter, name: &'static str) -> Option<proc_macro2::TokenStream> {
    input.next().and_then(|t| match t {
        TokenTree::Ident(ref i) if i.to_string() == name => {
            input.next(); // skip "="
            Some(
                input
                    .take_while(|tok| match tok {
                        TokenTree::Punct(_) => false,
                        _ => true,
                    })
                    .collect(),
            )
        }
        _ => None,
    })
}

fn root_crate_path() -> std::path::PathBuf {
    let path = env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is not set. Please use cargo to compile your crate.");
    let path = Path::new(&path);
    if path
        .parent()
        .expect("No parent dir")
        .join("Cargo.toml")
        .exists()
    {
        path.parent().expect("No parent dir").to_path_buf()
    } else {
        path.to_path_buf()
    }
}

struct Config {
    domain: String,
    make_po: bool,
    make_mo: bool,
    langs: Vec<String>,
}

impl Config {
    fn path() -> std::path::PathBuf {
        Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
            root_crate_path()
                .join("target")
                .join("debug")
                .to_str()
                .expect("Couldn't compute mo output dir")
                .into()
        }))
        .join("gettext_macros")
        .join(env::var("CARGO_PKG_NAME").expect("Please build with cargo"))
    }

    fn read() -> Config {
        let config = read(Config::path())
            .expect("Coudln't read domain, make sure to call init_i18n! before");
        let mut lines = config.lines();
        let domain = lines
            .next()
            .expect("Invalid config file. Make sure to call init_i18n! before this macro")
            .expect("IO error while reading config");
        let make_po: bool = lines
            .next()
            .expect("Invalid config file. Make sure to call init_i18n! before this macro")
            .expect("IO error while reading config")
            .parse()
            .expect("Couldn't parse make_po");
        let make_mo: bool = lines
            .next()
            .expect("Invalid config file. Make sure to call init_i18n! before this macro")
            .expect("IO error while reading config")
            .parse()
            .expect("Couldn't parse make_mo");
        Config {
            domain,
            make_po,
            make_mo,
            langs: lines
                .map(|l| l.expect("IO error while reading config"))
                .collect(),
        }
    }

    fn write(&self) {
        // emit file to include
        create_dir_all(Config::path().parent().unwrap()).expect("Couldn't create output dir");
        let mut out = File::create(Config::path()).expect("Metadata file couldn't be open");
        writeln!(out, "{}", self.domain).expect("Couldn't write domain");
        writeln!(out, "{}", self.make_po).expect("Couldn't write po settings");
        writeln!(out, "{}", self.make_mo).expect("Couldn't write mo settings");
        for l in self.langs.clone() {
            writeln!(out, "{}", l).expect("Couldn't write lang");
        }
    }
}

trait Message {
    fn writable(&self) -> bool;
    fn content(&self) -> String;
    fn context(&self) -> Option<String>;
    fn plural(&self) -> Option<String>;

    fn write(&self) {
        if !self.writable() {
            return;
        }

        let config = Config::read();

        let mut pot = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("po/{0}/{0}.pot", config.domain))
            .expect("Couldn't open .pot file");

        let mut contents = String::new();
        pot.read_to_string(&mut contents)
            .expect("IO error while reading .pot file");
        pot.seek(SeekFrom::End(0))
            .expect("IO error while seeking .pot file to end");

        let already_exists = self.content().is_empty()
            || contents.contains(&format!(
                r#"{}msgid "{}""#,
                self.context()
                    .clone()
                    .map(|c| format!(
r#"msgctxt "{}"
"#,
                    c))
                    .unwrap_or_default(),
                self.content()
            ));
        if already_exists {
            return;
        }

        let prefix = if let Some(c) = self.context() {
            format!(
r#"msgctxt "{}"
"#, c)
        } else {
            String::new()
        };

        if let Some(ref pl) = self.plural() {
            pot.write_all(
                &format!(
                    r#"
{}msgid "{}"
msgid_plural "{}"
msgstr[0] ""
"#,
                    prefix, self.content(), pl,
                )
                .into_bytes(),
            )
            .expect("Couldn't write message to .pot (plural)");
        } else {
            pot.write_all(
                &format!(
                    r#"
{}msgid "{}"
msgstr ""
"#,
                    prefix, self.content(),
                )
                .into_bytes(),
            )
            .expect("Couldn't write message to .pot");
        }
    }
}

struct I18nCall {
    catalog: syn::Expr,
    context: Option<syn::LitStr>,
    msg: syn::Expr,
    plural: Option<syn::Expr>,
    format_args: Option<syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>>,
}

mod kw {
    syn::custom_keyword!(context);
}

impl syn::parse::Parse for I18nCall {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let catalog = input.parse()?;
        input.parse::<Token![,]>()?;
        let context = if input.parse::<kw::context>().is_ok() {
            input.parse::<Token![=]>()?;
            let ctx = input.parse().ok();
            input.parse::<Token![,]>()?;
            ctx
        } else {
            None
        };
        let msg = input.parse()?;
        let plural = if input.parse::<Token![,]>().is_ok() {
            input.parse().ok()
        } else {
            None
        };
        let format_args = if input.parse::<Token![;]>().is_ok() {
            syn::punctuated::Punctuated::parse_terminated(input).ok()
        } else {
            None
        };

        Ok(I18nCall {
            catalog,
            context,
            msg,
            plural,
            format_args,
        })
    }
}

fn extract_str_lit(expr: &syn::Expr) -> Option<String> {
    match *expr {
        syn::Expr::Lit(syn::ExprLit { lit : syn::Lit::Str(ref s), attrs: _ }) => Some(s.value()),
        _ => None,
    }
}

impl Message for I18nCall {
    fn writable(&self) -> bool {
        extract_str_lit(&self.msg).is_some()
    }

    fn content(&self) -> String {
        extract_str_lit(&self.msg).unwrap_or_default().replace("\"", "\\\"").replace('\n', "\\n")
    }

    fn context(&self) -> Option<String> {
        self.context.as_ref().map(|c| c.value())
    }

    fn plural(&self) -> Option<String> {
        self.plural.as_ref().and_then(extract_str_lit)
    }
}

struct TCall {
    context: Option<syn::LitStr>,
    msg: syn::LitStr,
    plural: Option<syn::LitStr>,
}

impl syn::parse::Parse for TCall {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let context = if input.parse::<kw::context>().is_ok() {
            input.parse::<Token![=]>()?;
            let ctx = input.parse().ok();
            input.parse::<Token![,]>()?;
            ctx
        } else {
            None
        };

        let msg = input.parse()?;
        let plural = if input.parse::<Token![,]>().is_ok() {
            input.parse().ok()
        } else {
            None
        };

        Ok(TCall {
            context,
            msg,
            plural,
        })
    }
}

impl Message for TCall {
    fn writable(&self) -> bool {
        true
    }

    fn content(&self) -> String {
        self.msg.value().replace("\"", "\\\"").replace('\n', "\\n")
    }

    fn context(&self) -> Option<String> {
        self.context.as_ref().map(|c| c.value())
    }

    fn plural(&self) -> Option<String> {
        self.plural.as_ref().map(|p| p.value())
    }
}

/// Marks a string as translatable
///
/// It only adds the given string to the `.pot` file, without translating it at runtime.
///
/// To translate it for real, you will have to use `i18n`. The advantage of this macro, is
/// that you mark a string as translatable without requiring a catalog to be available in scope.
///
/// # Return value
///
/// In case of a singular message, the message itself is returned.
///
/// For messages with a plural form, it is a tuple containing the singular form, and the plural one.
///
/// # Example
///
/// ```rust,ignore
/// #use gettext_macros::*;
/// // Let's say we can't have access to a Catalog at this point of the program
/// let msg = t!("Hello, world!");
/// let plural = t!("Singular", "Plural")
///
/// // Now, let's get a catalog, and translate these messages
/// let cat = get_catalog();
/// i18n!(cat, msg);
/// i18n!(cat, plural.0, plural.1; 57);
/// ```
///
/// # Syntax
///
/// This macro accepts the following syntaxes:
///
/// ```rust,ignore
/// t!($singular)
/// t!($singular, $plural)
/// t!(context = $ctx, $singular)
/// t!(context = $ctx, $singular, $plural)
/// ```
///
/// Where `$singular`, `$plural` and `$ctx` all are `str` literals (and not variables, expressions or literal of any other type).
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    let message = syn::parse_macro_input!(input as TCall);
    message.write();
    let msg = message.content();
    if let Some(pl) = message.plural.clone() {
        quote!(
            (#msg, #pl)
        ).into()
    } else {
        quote!(#msg).into()
    }
}

/// Marks a string as translatable and translate it at runtime.
///
/// It add the string to the `.pot` file and translate them at runtime, using a given `gettext::Catalog`.
///
/// # Return value
///
/// This macro returns the translated string.
///
/// # Panics
///
/// This macro will panic if it the format string (of the translation) does not match the
/// format arguments that were given. For instance, if you have a string `Hello!`, that
/// is translated in Esperanto as `Saluton {name}!`, and that you call this function without
/// any format argument (as expected in the original English string), it will panic.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust,ignore
/// // cat is the gettext::Catalog containing translations for the current locale.
/// let cat = get_catalog();
/// i18n!(cat, "Hello, world!");
/// ```
///
/// Formatting a translated string:
///
/// ```rust,ignore
/// let name = "Peter";
/// i18n!(cat, "Hi {0}!"; name);
///
/// // Also works with multiple format arguments
/// i18n!(cat, "You are our {}th visitor! You won ${}!"; 99_999, 2);
/// ```
///
/// With a context, that will be shown to translators:
///
/// ```rust,ignore
/// let name = "Sophia";
/// i18n!(cat, context = "The variable is the name of the person being greeted", "Hello, {0}!"; name);
/// ```
///
/// Translating string that changes depending on a number:
///
/// ```rust,ignore
/// let flowers_count = 18;
/// i18n!(cat, "What a nice flower!", "What a nice garden!"; flowers_count);
/// ```
///
/// With all available options:
///
/// ```rust,ignore
/// let updates = 69;
/// i18n!(
///     cat,
///     context = "The notification when updates are available.",
///     "There is {} app update available."
///     "There are {} app updates available.";
///     updates
/// );
/// ```
///
/// # Syntax
///
/// This macro expects:
///
/// - first, the expression to get the translation catalog to use
/// - then, optionally, the `context` named argument, that is a string that will be shown
///   to translators. It should be a `str` literal, because it needs to be known at compile time.
/// - the message to translate. It can either be a string literal, or an expression, but if you use the later
///   make sure that the string is correctly added to the `.pot` file with `t`.
/// - if this message has a plural version, it should come after. Here too, both string literals or other expressions
///   are allowed
///
/// All these arguments should be separated by commas.
///
/// If you want to pass format arguments to this macro, to have them inserted into the translated strings,
/// you should add them at the end, after a colon, and seperate them with commas too.
#[proc_macro]
pub fn i18n(input: TokenStream) -> TokenStream {
    let message = syn::parse_macro_input!(input as I18nCall);
    message.write();

    let gettext_call = message.catalog.clone();
    let content = message.msg;
    let gettext_call = if let Some(pl) = message.plural {
        let count = message
            .format_args
            .clone()
            .and_then(|args| args.first().cloned());
        if let Some(c) = message.context {
            quote!(
                #gettext_call.npgettext(#c, #content, #pl, #count as u64)
            )
        } else {
            quote!(
                #gettext_call.ngettext(#content, #pl, #count as u64)
            )
        }
    } else {
        if let Some(c) = message.context {
            quote!(
                #gettext_call.pgettext(#c, #content)
            )
        } else {
            quote!(
                #gettext_call.gettext(#content)
            )
        }
    };

    let fargs: syn::punctuated::Punctuated<proc_macro2::TokenStream, Token![,]> = message.format_args.unwrap_or_default().into_iter().map(|x| {
        quote!(::std::boxed::Box::new(#x))
    }).collect();
    let res = quote!({
        use gettext_utils::try_format;
        try_format(#gettext_call, &[#fargs]).expect("Error while formatting message")
    });
    res.into()
}

/// This macro configures internationalization for the current crate
///
/// This macro expands to nothing, it just write your configuration to files
/// for other macros calls, and creates the `.pot` file if needed.
///
/// This macro should be called before (not in the program flow, but in the Rust parser flow) all other
/// internationalization macros.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust,ignore
/// init_i18n!("my_app", de, en, eo, fr, ja, pl, ru);
/// ```
/// With `.po` and `.mo` generation turned off, and without comments about string location in the `.pot`:
///
/// ```rust,ignore
/// init_i18n!("my_app", po = false, mo = false, de, en, eo, fr, ja, pl, ru);
/// ```
///
/// # Syntax
///
/// This macro expects:
///
/// - a string literal, that is the translation domain of your crate.
/// - optionally, the `po` named argument, that is a boolean literal to turn off `.po` generation from `.pot` in `compile_i18n`
/// - optionally, the `mo` named argument, that is a boolean literal too, to turn of `.po` compilation into `.mo` files in `compile_i18n`.
///   Note that if you turn this feature off, `include_i18n` won't work unless you manually generate the `.mo` files in
///   `target/TARGET/gettext_macros/LOCALE/DOMAIN.mo`.
/// - optionally, the `location` named argument, a boolean too, to avoid writing the location of the string in the source code to translation files.
///   Having this location available can be usefull if your translators know a bit of Rust and needs context about what they are translating, but it
///   also makes bigger diffs, because your `.pot` and `.po` files may be regenerated if a line number changes.
/// - then, the list of languages you want your app to be translated in, separated by commas. The languages are not string literals, but identifiers.
///
/// All the three boolean options are turned on by default. Also note that you may ommit one (or more) of them, but they should always be in this order.
#[proc_macro]
pub fn init_i18n(input: TokenStream) -> TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let mut input = input.into_iter();
    let domain = match input.next() {
        Some(TokenTree::Literal(lit)) => lit.to_string().replace("\"", ""),
        Some(_) => panic!("Domain should be a str"),
        None => panic!("Expected a translation domain (for instance \"myapp\")"),
    };

    let (po, mo) = if let Some(n) = input.next() {
        if is(&n, ',') {
            let po = named_arg(input.clone(), "po");
            if let Some(po) = po.clone() {
                for _ in 0..(po.into_iter().count() + 3) {
                    input.next();
                }
            }

            let mo = named_arg(input.clone(), "mo");
            if let Some(mo) = mo.clone() {
                for _ in 0..(mo.into_iter().count() + 3) {
                    input.next();
                }
            }

            (po, mo)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let mut langs = vec![];
    match input.next() {
        Some(TokenTree::Ident(i)) => {
            langs.push(i.to_string());
            loop {
                let next = input.next();
                if next.is_none() || !is(&next.expect("Unreachable: next should be Some"), ',') {
                    break;
                }
                match input.next() {
                    Some(TokenTree::Ident(i)) => {
                        langs.push(i.to_string());
                    }
                    _ => panic!("Expected a language identifier"),
                }
            }
        }
        None => {}
        _ => panic!("Expected a language identifier"),
    }

    let conf = Config {
        domain: domain.clone(),
        make_po: po.map(|x| x.to_string() == "true").unwrap_or(true),
        make_mo: mo.map(|x| x.to_string() == "true").unwrap_or(true),
        langs,
    };
    conf.write();

    // write base .pot
    create_dir_all(format!("po/{}", domain)).expect("Couldn't create po dir");
    let mut pot = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("po/{0}/{0}.pot", domain))
        .expect("Couldn't open .pot file");
    pot.write_all(
        &format!(
            r#"msgid ""
msgstr ""
"Project-Id-Version: {}\n"
"Report-Msgid-Bugs-To: \n"
"POT-Creation-Date: 2018-06-15 16:33-0700\n"
"PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\n"
"Last-Translator: FULL NAME <EMAIL@ADDRESS>\n"
"Language-Team: LANGUAGE <LL@li.org>\n"
"Language: \n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=INTEGER; plural=EXPRESSION;\n"
"#,
            domain
        )
        .into_bytes(),
    )
    .expect("Couldn't init .pot file");

    quote!().into()
}

/// Gives you the translation domain for the current crate.
///
/// # Return value
///
/// A `'static str` containing the GetText domain of this crate.
///
/// # Example
///
/// ```rust,ignore
/// println!("The GetText domain is: {}", i18n_domain!());
/// ```
#[proc_macro]
pub fn i18n_domain(_: TokenStream) -> TokenStream {
    let domain = Config::read().domain;
    let tok = TokenTree::Literal(Literal::string(&domain));
    quote!(#tok).into()
}

/// Compiles your internationalization files.
///
/// This macro expands to nothing, it just writes `.po` and `.mo` files.
///
/// You can configure its behavior with the `po` and `mo` options of `init_i18n`.
///
/// This macro should be called after (not in the program flow, but in the Rust parser flow) all other internationlaziton macros,
/// expected `include_i18n`.
///
/// # Example
///
/// ```rust,ignore
/// compile_i18n!();
/// ```
#[proc_macro]
pub fn compile_i18n(_: TokenStream) -> TokenStream {
    let conf = Config::read();
    let domain = &conf.domain;

    let pot_path = root_crate_path()
        .join("po")
        .join(domain.clone())
        .join(format!("{}.pot", domain));

    for lang in conf.langs {
        let po_path = root_crate_path()
            .join("po")
            .join(domain.clone())
            .join(format!("{}.po", lang.clone()));
        if conf.make_po {
            if po_path.exists() && po_path.is_file() {
                // Update it
                Command::new("msgmerge")
                    .arg("-U")
                    .arg(po_path.to_str().expect("msgmerge: PO path error"))
                    .arg(pot_path.to_str().expect("msgmerge: POT path error"))
                    .stdout(Stdio::null())
                    .status()
                    .map(|s| {
                        if !s.success() {
                            panic!("Couldn't update PO file")
                        }
                    })
                    .expect("Couldn't update PO file. Make sure msgmerge is installed.");
            } else {
                println!("Creating {}", lang.clone());
                // Create it from the template
                Command::new("msginit")
                    .arg(format!(
                        "--input={}",
                        pot_path.to_str().expect("msginit: POT path error")
                    ))
                    .arg(format!(
                        "--output-file={}",
                        po_path.to_str().expect("msginit: PO path error")
                    ))
                    .arg("-l")
                    .arg(lang.clone())
                    .arg("--no-translator")
                    .stdout(Stdio::null())
                    .status()
                    .map(|s| {
                        if !s.success() {
                            panic!("Couldn't init PO file (gettext returned an error)")
                        }
                    })
                    .expect("Couldn't init PO file. Make sure msginit is installed.");
            }
        }

        if conf.make_mo {
            if !po_path.exists() {
                panic!(
                    "{} doesn't exist. Make sure you didn't disabled po generation.",
                    po_path.display()
                );
            }

            // Generate .mo
            let mo_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
                root_crate_path()
                    .join("target")
                    .join("debug")
                    .to_str()
                    .expect("Couldn't compute mo output dir")
                    .into()
            }))
            .join("gettext_macros")
            .join(lang);
            create_dir_all(mo_dir.clone()).expect("Couldn't create MO directory");
            let mo_path = mo_dir.join(format!("{}.mo", domain));

            Command::new("msgfmt")
                .arg(format!(
                    "--output-file={}",
                    mo_path.to_str().expect("msgfmt: MO path error")
                ))
                .arg(po_path)
                .stdout(Stdio::null())
                .status()
                .map(|s| {
                    if !s.success() {
                        panic!("Couldn't compile translations (gettext returned an error)")
                    }
                })
                .expect("Couldn't compile translations. Make sure msgfmt is installed");
        }
    }
    quote!().into()
}

/// Use this macro to staticaly import translations into your final binary.
///
/// This macro won't work if ou set `mo = false` in `init_i18n`, unless you manually generate the `.mo` files in
/// `target/TARGET/gettext_macros/LOCALE/DOMAIN.mo`.
///
/// # Example
///
/// ```rust,ignore
/// let catalogs = include_i18n!();
/// catalog.into_iter()
///     .find(|(lang, _)| lang == "eo")
///     .map(|(_, catalog| println!("{}", i18n!(catalog, "Hello world!")));
/// ```
#[proc_macro]
pub fn include_i18n(_: TokenStream) -> TokenStream {
    let conf = Config::read();
    let locales = conf.langs.clone().into_iter().map(|l| {
        let lang = TokenTree::Literal(Literal::string(&l));
        let path = Config::path().parent().unwrap().join(l).join(format!("{}.mo", conf.domain));

        if !path.exists() {
            panic!("{} doesn't exist. Make sure to call compile_i18n! before include_i18n!, and check that you didn't disabled mo compilation.", path.display());
        }

        let path = TokenTree::Literal(Literal::string(path.to_str().expect("Couldn't write MO file path")));
        quote!{
            (#lang, ::gettext::Catalog::parse(
                &include_bytes!(
                    #path
                )[..]
            ).expect("Error while loading catalog")),
        }
	}).collect::<proc_macro2::TokenStream>();

    quote!({
        vec![
            #locales
        ]
    }).into()
}
