use crate::users::User;
use rocket::{
    http::Status,
    request::{self, FromRequestAsync, Request},
    Outcome,
};

/// Wrapper around User to use as a request guard on pages reserved to admins.
pub struct Admin(pub User);

impl<'a, 'r> FromRequestAsync<'a, 'r> for Admin {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::FromRequestFuture<'a, Self, Self::Error> {
        Box::pin(async move {
            let user = try_outcome!(request.guard::<User>());
            if user.is_admin() {
                Outcome::Success(Admin(user))
            } else {
                Outcome::Failure((Status::Unauthorized, ()))
            }
        })
    }
}

/// Same as `Admin` but for moderators.
pub struct Moderator(pub User);

impl<'a, 'r> FromRequestAsync<'a, 'r> for Moderator {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::FromRequestFuture<'a, Self, Self::Error> {
        Box::pin(async move {
            let user = try_outcome!(request.guard::<User>());
            if user.is_moderator() {
                Outcome::Success(Moderator(user))
            } else {
                Outcome::Failure((Status::Unauthorized, ()))
            }
        })
    }
}
