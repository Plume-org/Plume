use rocket::{
    http::{RawStr, uri::Uri},
    response::{Flash, Redirect},
};

/**
* Redirects to the login page with a given message.
*
* Note that the message should be translated before passed to this function.
*/
pub fn requires_login<T: Into<Uri<'static>>>(message: &str, url: T) -> Flash<Redirect> {
    Flash::new(
        Redirect::to(format!("/login?m={}", RawStr::new(&message).percent_encode().to_string())),
        "callback",
        url.into().to_string(),
    )
}
