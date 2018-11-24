use rocket::response::{Redirect, Flash};
use rocket_contrib::templates::Template;

use plume_common::utils;
use plume_models::{db_conn::DbConn, notifications::Notification, users::User};
use routes::Page;

#[get("/notifications?<page>")]
pub fn paginated_notifications(conn: DbConn, user: User, page: Page) -> Template {
    Template::render("notifications/index", json!({
        "account": user.to_json(&*conn),
        "notifications": Notification::page_for_user(&*conn, &user, page.limits()).into_iter().map(|n| n.to_json(&*conn)).collect::<Vec<_>>(),
        "page": page.0,
        "n_pages": Page::total(Notification::find_for_user(&*conn, &user).len() as i32)
    }))
}

#[get("/notifications")]
pub fn notifications(conn: DbConn, user: User) -> Template {
    paginated_notifications(conn, user, Page::first())
}

#[get("/notifications", rank = 2)]
pub fn notifications_auth() -> Flash<Redirect>{
    utils::requires_login(
        "You need to be logged in order to see your notifications",
        uri!(notifications)
    )
}
