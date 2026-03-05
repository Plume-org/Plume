use crate::users::User;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome, Request},
};

/// Wrapper around User to use as a request guard on pages exclusively reserved to admins.
pub struct Admin(pub User);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Admin {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Admin, Self::Error> {
        match request.guard::<User>().await {
            Outcome::Success(user) if user.is_admin() => Outcome::Success(Admin(user)),
            _ => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}

/// Same as `Admin` but it forwards to next guard if the user is not an admin.
/// It's useful when there are multiple implementations of routes for admin and moderator.
pub struct InclusiveAdmin(pub User);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for InclusiveAdmin {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<InclusiveAdmin, Self::Error> {
        match request.guard::<User>().await {
            Outcome::Success(user) if user.is_admin() => Outcome::Success(InclusiveAdmin(user)),
            _ => Outcome::Forward(Status::Unauthorized)
        }
    }
}

/// Same as `Admin` but for moderators.
pub struct Moderator(pub User);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Moderator {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Moderator, Self::Error> {
        match request.guard::<User>().await {
            Outcome::Success(user) if user.is_moderator() => Outcome::Success(Moderator(user)),
            _ => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}
