use rocket_contrib::Template;
use serde_json;

use plume_models::{
    db_conn::DbConn,
    posts::Post,
    tags::Tag,
    users::User,
};
use routes::Page;

#[get("/tag/<name>")]
fn tag(user: Option<User>, conn: DbConn, name: String) -> Template {
    paginated_tag(user, conn, name, Page::first())
}

#[get("/tag/<name>?<page>")]
fn paginated_tag(user: Option<User>, conn: DbConn, name: String, page: Page) -> Template {
    let tag = Tag::find_by_name(&*conn, name).expect("Rendering tags::tag: tag not found");
    let posts = Post::list_by_tag(&*conn, tag.tag.clone(), page.limits());
    Template::render("tags/index", json!({
        "tag": tag.clone(),
        "account": user.map(|u| u.to_json(&*conn)),
        "articles": posts.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
        "page": page.page,
        "n_pages": Page::total(Post::count_for_tag(&*conn, tag.tag) as i32)
    }))
}
