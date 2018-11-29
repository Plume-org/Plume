use chrono::offset::Utc;
use rocket_contrib::Template;
use serde_json;

use plume_models::{
    db_conn::DbConn, users::User,
    search::Query};
use routes::Page;
use Searcher;

#[get("/search")]
fn index(conn: DbConn, user: Option<User>) -> Template {
    Template::render("search/index", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "now": format!("{}", Utc::today().format("%Y-%m-d")),
    }))
}

#[derive(FromForm)]
struct SearchQuery {
    q: Option<String>,
    title: Option<String>,
    subtitle: Option<String>,
    content: Option<String>,
    instance: Option<String>,
    author: Option<String>,
    tag: Option<String>,
    blog: Option<String>,
    lang: Option<String>,
    license: Option<String>,
    after: Option<String>,
    before: Option<String>,
    page: Option<Page>,
}

macro_rules! param_to_query {
    ( $query:ident, $parsed_query:ident; normal: $($field:ident),*; date: $($date:ident),*) => {
        $(
            if let Some(field) = $query.$field {
                let mut rest = field.as_str();
                while !rest.is_empty() {
                    let (token, r) = Query::get_first_token(rest);
                    rest = r;
                    $parsed_query.$field(token, None);
                }
            }
        )*
        $(
            if let Some(field) = $query.$date {
                let mut rest = field.as_str();
                while !rest.is_empty() {
                    use chrono::naive::NaiveDate;
                    let (token, r) = Query::get_first_token(rest);
                    rest = r;
                    if let Ok(token) = NaiveDate::parse_from_str(token, "%Y-%m-%d") {
                        $parsed_query.$date(&token);
                    }
                }
            }
        )*
    }
}


#[get("/search?<query>")]
fn query(query: SearchQuery, conn: DbConn, searcher: Searcher, user: Option<User>) -> Template {
    let page = query.page.unwrap_or(Page::first());
    let mut parsed_query = Query::from_str(&query.q.unwrap_or_default());

    param_to_query!(query, parsed_query; normal: title, subtitle, content, tag,
              instance, author, blog, lang, license;
              date: before, after);

    let str_q = parsed_query.to_string();
    let res = searcher.search_document(&conn, parsed_query, page.limits());

    Template::render("search/result", json!({
        "query":str_q,
        "account": user.map(|u| u.to_json(&*conn)),
        "next_page": if res.is_empty() { 0 } else { page.page+1 },
        "posts": res.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
        "page": page.page,
    }))
}
