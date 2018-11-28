use rocket_i18n::I18n;

use plume_models::{
    db_conn::DbConn,
    posts::Post,
    users::User,
};
use routes::{Ructe, Page};

#[get("/tag/<name>")]
pub fn tag(user: Option<User>, conn: DbConn, name: String, intl: I18n) -> Ructe {
    paginated_tag(user, conn, name, Page::first(), intl)
}

#[get("/tag/<name>?<page>")]
pub fn paginated_tag(user: Option<User>, conn: DbConn, name: String, page: Page, intl: I18n) -> Ructe {
    let posts = Post::list_by_tag(&*conn, name.clone(), page.limits());
    render!(tags::index(
        &(&*conn, &intl.catalog, user),
        name.clone(),
        posts,
        page.0,
        Page::total(Post::count_for_tag(&*conn, name) as i32)
    ))
}
