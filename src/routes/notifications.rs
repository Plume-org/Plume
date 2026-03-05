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
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let page_total = Page::total(Notification::count_for_user(&mut conn, &user)? as i32);

    let notifs = Notification::page_for_user(&mut conn, &user, page.limits())?
        .into_iter()
        .map(|n| {
            let actor = n.get_actor(&mut conn).ok();
            let url = n.get_url(&mut conn);
            let post = n.get_post(&mut conn);
            let post_url = post.as_ref().and_then(|p| p.url(&mut conn).ok()).unwrap_or_default();
            (n, actor, url, post, post_url)
        })
        .collect();

    Ok(render!(notifications::index_html(
        &(&mut conn, &rockets).to_context(),
        notifs,
        page.0,
        page_total
    )))
}

#[get("/notifications?<page>", rank = 2)]
pub fn notifications_auth(i18n: I18n, page: Option<Page>) -> Flash<Redirect> {
    requires_login(
        &i18n!(
            i18n.catalog,
            "To see your notifications, you need to be logged in"
        ),
        uri!(notifications(page = page)),
    )
}
