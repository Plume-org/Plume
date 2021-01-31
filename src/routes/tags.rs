use crate::routes::{errors::ErrorPage, Page};
use crate::template_utils::{IntoContext, Ructe};
use plume_models::{db_conn::DbConn, posts::Post, PlumeRocket};

#[get("/tag/<name>?<page>")]
pub fn tag(
    name: String,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let posts = Post::list_by_tag(&conn, name.clone(), page.limits())?;
    Ok(render!(tags::index(
        &(&conn, &rockets).to_context(),
        name.clone(),
        posts,
        page.0,
        Page::total(Post::count_for_tag(&conn, name)? as i32)
    )))
}
