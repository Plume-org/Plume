use rocket::request::FlashMessage;
use rocket_i18n::I18n;

use plume_models::{db_conn::DbConn, posts::Post, users::User};
use routes::{errors::ErrorPage, Page};
use template_utils::Ructe;

#[get("/tag/<name>?<page>")]
pub fn tag(
    user: Option<User>,
    conn: DbConn,
    name: String,
    page: Option<Page>,
    intl: I18n,
    msg: Option<FlashMessage>,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let posts = Post::list_by_tag(&*conn, name.clone(), page.limits())?;
    Ok(render!(tags::index(
        &(&*conn, &intl.catalog, user, msg),
        name.clone(),
        posts,
        page.0,
        Page::total(Post::count_for_tag(&*conn, name)? as i32)
    )))
}
