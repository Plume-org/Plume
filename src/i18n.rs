use gettextrs::*;
use rocket::{Data, Request, Rocket, fairing::{Fairing, Info, Kind}};
use serde_json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use tera::{Tera, Error as TeraError};

const ACCEPT_LANG: &'static str = "Accept-Language";

pub struct I18n {
    domain: &'static str
}

impl I18n {
    pub fn new(domain: &'static str) -> I18n {
        I18n {
            domain: domain
        }
    }
}

impl Fairing for I18n {
    fn info(&self) -> Info {
        Info {
            name: "Gettext I18n",
            kind: Kind::Attach | Kind::Request
        }
    }

    fn on_attach(&self, rocket: Rocket) -> Result<Rocket, Rocket> {
        bindtextdomain(self.domain, fs::canonicalize(&PathBuf::from("./translations/")).unwrap().to_str().unwrap());
        textdomain(self.domain);
        Ok(rocket)
    }

    fn on_request(&self, request: &mut Request, _: &Data) {
        let lang = request
            .headers()
            .get_one(ACCEPT_LANG)
            .unwrap_or("en")
            .split(",")
            .nth(0)
            .unwrap_or("en");
        
        // We can't use setlocale(LocaleCategory::LcAll, lang), because it only accepts system-wide installed
        // locales (and most of the time there are only a few of them).
        // But, when we set the LANGUAGE environment variable, and an empty string as a second parameter to
        // setlocale, gettext will be smart enough to find a matching locale in the locally installed ones.
        env::set_var("LANGUAGE", lang);
        setlocale(LocaleCategory::LcAll, "");
    }
}

fn tera_gettext(msg: serde_json::Value, ctx: HashMap<String, serde_json::Value>) -> Result<serde_json::Value, TeraError> {
    let trans = gettext(msg.as_str().unwrap());
    Ok(serde_json::Value::String(Tera::one_off(trans.as_ref(), &ctx, false).unwrap_or(String::from(""))))
}

fn tera_ngettext(msg: serde_json::Value, ctx: HashMap<String, serde_json::Value>) -> Result<serde_json::Value, TeraError> {
    let trans = ngettext(
        ctx.get("singular").unwrap().as_str().unwrap(),
        msg.as_str().unwrap(),
        ctx.get("count").unwrap().as_u64().unwrap() as u32
    );
    Ok(serde_json::Value::String(Tera::one_off(trans.as_ref(), &ctx, false).unwrap_or(String::from(""))))
}

pub fn tera(t: &mut Tera) {
    t.register_filter("_", tera_gettext);
    t.register_filter("_n", tera_ngettext);
}
