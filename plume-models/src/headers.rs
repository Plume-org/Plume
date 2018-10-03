use rocket::request::{self, FromRequest, Request};
use rocket::{http::HeaderMap, Outcome};


pub struct Headers<'r>(pub HeaderMap<'r>);

impl<'a, 'r> FromRequest<'a, 'r> for Headers<'r> {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, ()> {
        let mut headers = HeaderMap::new();
        for header in request.headers().clone().into_iter() {
            headers.add(header);
        }
        Outcome::Success(Headers(headers))
    }
}
