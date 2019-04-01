use rocket::response::{Flash, Redirect};
use rocket_i18n::I18n;

use plume_common::utils;
use plume_models::{db_conn::DbConn, notifications::Notification, users::User};
use routes::{errors::ErrorPage, Page};
use template_utils::Ructe;

#[get("/notifications?<page>")]
pub fn notifications(
    conn: DbConn,
    user: User,
    page: Option<Page>,
    intl: I18n,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    Ok(render!(notifications::index(
        &(&*conn, &intl.catalog, Some(user.clone())),
        Notification::page_for_user(&*conn, &user, page.limits())?,
        page.0,
        Page::total(Notification::count_for_user(&*conn, &user)? as i32)
    )))
}

#[get("/notifications?<page>", rank = 2)]
pub fn notifications_auth(i18n: I18n, page: Option<Page>) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(
            i18n.catalog,
            "To see your notifications, you need to be logged in"
        ),
        uri!(notifications: page = page),
    )
}
