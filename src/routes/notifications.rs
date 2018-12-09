use rocket::response::{Redirect, Flash};
use rocket_i18n::I18n;

use plume_common::utils;
use plume_models::{db_conn::DbConn, notifications::Notification, users::User};
use routes::Page;
use template_utils::Ructe;

#[get("/notifications?<page>")]
pub fn paginated_notifications(conn: DbConn, user: User, page: Page, intl: I18n) -> Ructe {
    render!(notifications::index(
        &(&*conn, &intl.catalog, Some(user.clone())),
        Notification::page_for_user(&*conn, &user, page.limits()),
        page.0,
        Page::total(Notification::count_for_user(&*conn, &user) as i32)
    ))
}

#[get("/notifications")]
pub fn notifications(conn: DbConn, user: User, intl: I18n) -> Ructe {
    paginated_notifications(conn, user, Page::first(), intl)
}

#[get("/notifications", rank = 2)]
pub fn notifications_auth(i18n: I18n) -> Flash<Redirect>{
    utils::requires_login(
        i18n!(i18n.catalog, "You need to be logged in order to see your notifications"),
        uri!(notifications)
    )
}
