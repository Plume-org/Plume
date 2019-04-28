use plume_models::{posts::Post, PlumeRocket};
use routes::{errors::ErrorPage, Page};
use template_utils::{IntoContext, Ructe};

#[get("/tag/<name>?<page>")]
pub fn tag(name: String, page: Option<Page>, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let posts = Post::list_by_tag(&*rockets.conn, name.clone(), page.limits())?;
    Ok(render!(tags::index(
        &rockets.to_context(),
        name.clone(),
        posts,
        page.0,
        Page::total(Post::count_for_tag(&*rockets.conn, name)? as i32)
    )))
}
