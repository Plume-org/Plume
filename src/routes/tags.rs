use crate::routes::{errors::ErrorPage, Page};
use crate::template_utils::{IntoContext, PostCard, Ructe};
use plume_models::{db_conn::DbConn, posts::Post, PlumeRocket};

#[get("/tag/<name>?<page>")]
pub fn tag(
    name: &str,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let page_total = Page::total(Post::count_for_tag(&mut conn, name.to_string())? as i32);

    let list = Post::list_by_tag(&mut conn, name.to_string(), page.limits())?;
    let posts = PostCard::from_posts(&mut conn, list, &rockets.user);

    Ok(render!(tags::index_html(
        &(&mut conn, &rockets).to_context(),
        name.to_string(),
        posts,
        page.0,
        page_total
    )))
}
