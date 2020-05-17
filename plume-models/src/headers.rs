use rocket::request::{self, FromRequest, Request};
use rocket::{
    http::{Header, HeaderMap},
    Outcome,
};

pub struct Headers<'r>(pub HeaderMap<'r>);

#[rocket::async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for Headers<'r> {
    type Error = ();

    async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, ()> {
        let mut headers = HeaderMap::new();
        for header in request.headers().clone().into_iter() {
            headers.add(header);
        }
        let ori = request.uri();
        let uri = if let Some(query) = ori.query() {
            format!("{}?{}", ori.path(), query)
        } else {
            ori.path().to_owned()
        };
        headers.add(Header::new(
            "(request-target)",
            format!("{} {}", request.method().as_str().to_lowercase(), uri),
        ));
        Outcome::Success(Headers(headers))
    }
}
