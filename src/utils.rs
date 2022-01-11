use rocket::{
    http::uri::Uri,
    response::{Flash, Redirect},
};

/**
* Redirects to the login page with a given message.
*
* Note that the message should be translated before passed to this function.
*/
pub fn requires_login<T: Into<Uri<'static>>>(message: &str, url: T) -> Flash<Redirect> {
    Flash::new(
        Redirect::to(format!("/login?m={}", Uri::percent_encode(message))),
        "callback",
        url.into().to_string(),
    )
}
