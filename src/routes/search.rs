use chrono::offset::Utc;
use crate::routes::Page;
use crate::template_utils::{IntoContext, PostCard, Ructe};
use plume_models::{db_conn::DbConn, search::Query, PlumeRocket};
use rocket::http::uri::fmt::{Ignorable, Query as RocketQuery};
use std::str::FromStr;

#[derive(Default, FromForm)]
pub struct SearchQuery {
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

impl Ignorable<RocketQuery> for SearchQuery { }

macro_rules! param_to_query {
    ( $query:ident, $parsed_query:ident; normal: $($field:ident),*; date: $($date:ident),*) => {
        $(
            let mut rest = $query.$field.as_ref().map(String::as_str).unwrap_or_default();
            while !rest.is_empty() {
                let (token, r) = Query::get_first_token(rest);
                rest = r;
                $parsed_query.$field(token, None);
            }
        )*
        $(
            if let Some(ref field) = $query.$date {
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

#[get("/search?<query..>")]
pub fn search(query: SearchQuery, mut conn: DbConn, rockets: PlumeRocket) -> Ructe {
    let page = query.page.unwrap_or_default();
    let mut parsed_query =
        Query::from_str(query.q.as_deref().unwrap_or_default()).unwrap_or_default();

    param_to_query!(query, parsed_query; normal: title, subtitle, content, tag,
              instance, author, blog, lang, license;
              date: before, after);

    let str_query = parsed_query.to_string();

    if str_query.is_empty() {
        render!(search::index_html(
            &(&mut conn, &rockets).to_context(),
            &format!("{}", Utc::now().date_naive().format("%Y-%m-d"))
        ))
    } else {
        let docs = rockets
            .searcher
            .search_document(&mut conn, parsed_query, page.limits());
        let next_page = if docs.is_empty() { 0 } else { page.0 + 1 };

        let pc = PostCard::from_posts(&mut conn, docs, &rockets.user);

        render!(search::result_html(
            &(&mut conn, &rockets).to_context(),
            &str_query,
            pc,
            page.0,
            next_page
        ))
    }
}
