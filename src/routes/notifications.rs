use rocket::response::{Flash, Redirect};
use rocket_i18n::I18n;

use crate::routes::{errors::ErrorPage, Page};
use crate::template_utils::{IntoContext, Ructe};
use crate::utils::requires_login;
use plume_models::{db_conn::DbConn, notifications::Notification, users::User, PlumeRocket};

#[get("/notifications?<page>")]
pub fn notifications(
    user: User,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    Ok(render!(notifications::index(
        &(&conn, &rockets).to_context(),
        Notification::page_for_user(&conn, &user, page.limits())?,
        page.0,
        Page::total(Notification::count_for_user(&conn, &user)? as i32)
    )))
}

#[get("/notifications?<page>", rank = 2)]
pub fn notifications_auth(i18n: I18n, page: Option<Page>) -> Flash<Redirect> {
    requires_login(
        &i18n!(
            i18n.catalog,
            "To see your notifications, you need to be logged in"
        ),
        uri!(notifications: page = page),
    )
}
