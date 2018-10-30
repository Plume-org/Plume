use rocket::{
    Outcome,
    http::Status,
    request::{self, FromRequest, Request}
};
use std::marker::PhantomData;
use plume_models::{self, api_tokens::ApiToken};

// Actions
pub trait Action {
    fn to_str() -> &'static str;
}
pub struct Read;
impl Action for Read {
    fn to_str() -> &'static str {
        "read"
    }
}
pub struct Write;
impl Action for Write {
    fn to_str() -> &'static str {
        "write"
    }
}

// Scopes
pub trait Scope {
    fn to_str() -> &'static str;
}
impl Scope for plume_models::posts::Post {
    fn to_str() -> &'static str {
        "posts"
    }
}

pub struct Authorization<A, S> (pub ApiToken, PhantomData<(A, S)>);

impl<'a, 'r, A, S> FromRequest<'a, 'r> for Authorization<A, S>
where A: Action,
      S: Scope
{
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Authorization<A, S>, ()> {
        request.guard::<ApiToken>()
            .map_failure(|_| (Status::Unauthorized, ()))
            .and_then(|token| if token.can(A::to_str(), S::to_str()) {
                Outcome::Success(Authorization(token, PhantomData))
            } else {
                Outcome::Failure((Status::Unauthorized, ()))
            })
    }
}

