use gettextrs::*;
use rocket::{Data, Request, Rocket, fairing::{Fairing, Info, Kind}};
use serde_json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
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
        update_po(self.domain);
        compile_po(self.domain);

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

fn update_po(domain: &str) {
    let pot_path = Path::new("po").join(format!("{}.pot", domain));

    for lang in get_locales() {
        let po_path = Path::new("po").join(format!("{}.po", lang.clone()));
        if po_path.exists() && po_path.is_file() {
            println!("Updating {}", lang.clone());
            // Update it
            Command::new("msgmerge")
                .arg("-U")
                .arg(po_path.to_str().unwrap())
                .arg(pot_path.to_str().unwrap())
                .spawn()
                .expect("Couldn't update PO file");
        } else {
            println!("Creating {}", lang.clone());
            // Create it from the template
            Command::new("msginit")
                .arg(format!("--input={}", pot_path.to_str().unwrap()))
                .arg(format!("--output-file={}", po_path.to_str().unwrap()))
                .arg("-l")
                .arg(lang)
                .arg("--no-translator")
                .spawn()
                .expect("Couldn't init PO file");
        }
    }
}

fn compile_po(domain: &str) {
    for lang in get_locales() {
        let po_path = Path::new("po").join(format!("{}.po", lang.clone()));
        let mo_dir = Path::new("translations")
            .join(lang.clone())
            .join("LC_MESSAGES");
        fs::create_dir_all(mo_dir.clone()).expect("Couldn't create MO directory");
        let mo_path = mo_dir.join(format!("{}.mo", domain));

        Command::new("msgfmt")
            .arg(format!("--output-file={}", mo_path.to_str().unwrap()))
            .arg(po_path)
            .spawn()
            .expect("Couldn't compile translations");
    }
}

fn get_locales() -> Vec<String> {
    let linguas_file = fs::File::open(Path::new("po").join("LINGUAS")).expect("Couldn't find po/LINGUAS file");
    let linguas = BufReader::new(&linguas_file);
    linguas.lines().map(Result::unwrap).collect()
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
