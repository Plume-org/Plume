use plume_models::{self, api_tokens::ApiToken};
use rocket::{
    http::Status,
    request::{self, FromRequest, Outcome, Request},
};
use std::marker::PhantomData;

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

pub struct Authorization<A, S>(pub ApiToken, PhantomData<(A, S)>);

#[rocket::async_trait]
impl<'r, A, S> FromRequest<'r> for Authorization<A, S>
where
    A: Action,
    S: Scope,
{
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Authorization<A, S>, ()> {
        request.guard::<ApiToken>().await
            .map_error(|_| (Status::Unauthorized, ()))
            .and_then(|token| {
                if token.can(A::to_str(), S::to_str()) {
                    Outcome::Success(Authorization(token, PhantomData))
                } else {
                    Outcome::Error((Status::Unauthorized, ()))
                }
            })
    }
}
