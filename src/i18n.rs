use gettextrs::*;
use rocket::{Data, Request, Rocket, fairing::{Fairing, Info, Kind}};

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
        bindtextdomain(self.domain, "/usr/local/share/locale");
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
        setlocale(LocaleCategory::LcAll, format!("{}.UTF-8", lang.replace("-", "_")));
    }
}
