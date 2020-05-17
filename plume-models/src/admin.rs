use crate::users::User;
use rocket::{
    http::Status,
    request::{self, FromRequest, Request},
    Outcome,
};

/// Wrapper around User to use as a request guard on pages reserved to admins.
pub struct Admin(pub User);

#[rocket::async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for Admin {
    type Error = ();

    async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let user = try_outcome!(User::from_request(request).await);
        if user.is_admin() {
            Outcome::Success(Admin(user))
        } else {
            Outcome::Failure((Status::Unauthorized, ()))
        }
    }
}

/// Same as `Admin` but for moderators.
pub struct Moderator(pub User);

#[rocket::async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for Moderator {
    type Error = ();

    async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let user = try_outcome!(User::from_request(request).await);
        if user.is_moderator() {
            Outcome::Success(Moderator(user))
        } else {
            Outcome::Failure((Status::Unauthorized, ()))
        }
    }
}
