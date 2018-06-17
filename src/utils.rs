use gettextrs::gettext;
use heck::CamelCase;
use rocket::{
    http::uri::Uri,
    response::{Redirect, Flash}
};

/// Remove non alphanumeric characters and CamelCase a string
pub fn make_actor_id(name: String) -> String {
    name.as_str()
        .to_camel_case()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn requires_login(message: &str, url: &str) -> Flash<Redirect> {
    Flash::new(Redirect::to(Uri::new(format!("/login?m={}", gettext(message.to_string())))), "callback", url)
}
