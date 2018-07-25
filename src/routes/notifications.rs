use rocket::response::{Redirect, Flash};
use rocket_contrib::Template;

use plume_common::utils;
use plume_models::{db_conn::DbConn, notifications::Notification, users::User};
use routes::Page;

#[get("/notifications?<page>")]
fn paginated_notifications(conn: DbConn, user: User, page: Page) -> Template {
    Template::render("notifications/index", json!({
        "account": user,
        "notifications": Notification::page_for_user(&*conn, &user, page.limits()),
        "page": page.page,
        "n_pages": Page::total(Notification::find_for_user(&*conn, &user).len() as i32)
    }))
}

#[get("/notifications")]
fn notifications(conn: DbConn, user: User) -> Template {
    paginated_notifications(conn, user, Page::first())
}

#[get("/notifications", rank = 2)]
fn notifications_auth() -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to see your notifications", uri!(notifications))
}
