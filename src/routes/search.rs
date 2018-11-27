use rocket_contrib::Template;
use serde_json;

use plume_models::{
    db_conn::DbConn, users::User};
use routes::Page;
use Searcher;

#[get("/search")]
fn index(conn: DbConn, user: Option<User>) -> Template {
    Template::render("search/index", json!({
        "account": user.map(|u| u.to_json(&*conn)),
    }))
}

#[derive(FromForm)]
struct SearchQuery {
    q: String,
    page: Option<Page>,
}

#[get("/search?<query>")]
fn query(query: SearchQuery, conn: DbConn, searcher: Searcher, user: Option<User>) -> Template {
    let page = query.page.unwrap_or(Page::first());
    let res = searcher.search_document(&conn, &query.q, page.limits());

    Template::render("search/result", json!({
        "query": query.q,
        "account": user.map(|u| u.to_json(&*conn)),
        "next_page": if res.is_empty() { 0 } else { page.page+1 },
        "posts": res.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
        "page": page.page,
    }))
}
