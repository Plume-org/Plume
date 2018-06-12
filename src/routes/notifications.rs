use rocket::response::{Redirect, Flash};
use rocket_contrib::Template;

use db_conn::DbConn;
use models::{notifications::Notification, users::User};

use utils;

#[get("/notifications")]
fn notifications(conn: DbConn, user: User) -> Template {
    Template::render("notifications/index", json!({
        "account": user,
        "notifications": Notification::find_for_user(&*conn, &user)
    }))
}

#[get("/notifications", rank = 2)]
fn notifications_auth() -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to see your notifications", "/notifications")
}
