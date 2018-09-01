use rocket::{Outcome, http::Status, request::{self, FromRequest, Request}};

use users::User;

/// Wrapper around User to use as a request guard on pages reserved to admins.
pub struct Admin(pub User);

impl<'a, 'r> FromRequest<'a, 'r> for Admin {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Admin, ()> {
        let user = request.guard::<User>()?;
        if user.is_admin {
            Outcome::Success(Admin(user))
        } else {
            Outcome::Failure((Status::Unauthorized, ()))
        }
    }
}
