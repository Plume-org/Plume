use rocket::response::{Redirect, Flash};
use rocket_contrib::Template;

use plume_common::utils;
use plume_models::{db_conn::DbConn, notifications::Notification, users::User};

#[get("/notifications")]
fn notifications(conn: DbConn, user: User) -> Template {
    Template::render("notifications/index", json!({
        "account": user,
        "notifications": Notification::find_for_user(&*conn, &user)
    }))
}

#[get("/notifications", rank = 2)]
fn notifications_auth() -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to see your notifications", uri!(notifications))
}
