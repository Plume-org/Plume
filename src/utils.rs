use rocket::{
    fairing::{Fairing, Info, Kind},
    http::uri::Uri,
    response::{Flash, Redirect},
    Request, Response,
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

/**
* Checks that a user-supplied URI (for instance a remote interaction
* endpoint obtained via WebFinger) can safely be used as a redirect target.
*/
pub fn is_safe_redirect_uri(uri: &str) -> bool {
    plume_common::activity_pub::request::is_safe_url_str(uri)
}

/// Adds a few defense-in-depth security headers to every response.
pub struct SecurityHeaders;

impl Fairing for SecurityHeaders {
    fn info(&self) -> Info {
        Info {
            name: "Security headers",
            kind: Kind::Response,
        }
    }

    fn on_response(&self, _request: &Request<'_>, response: &mut Response<'_>) {
        response.set_raw_header("X-Content-Type-Options", "nosniff");
        response.set_raw_header("X-Frame-Options", "SAMEORIGIN");
        response.set_raw_header("Referrer-Policy", "same-origin");
    }
}
