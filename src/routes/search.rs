use chrono::offset::Utc;
use rocket::request::Form;

use plume_models::{search::Query, PlumeRocket};
use routes::Page;
use std::str::FromStr;
use template_utils::{IntoContext, Ructe};

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

fn search_guts(query: Option<Form<SearchQuery>>, rockets: PlumeRocket) -> Ructe {
    let conn = &*rockets.conn;
    let query = query.map(Form::into_inner).unwrap_or_default();
    let page = query.page.unwrap_or_default();
    let mut parsed_query =
        Query::from_str(&query.q.as_ref().map(String::as_str).unwrap_or_default())
            .unwrap_or_default();

    param_to_query!(query, parsed_query; normal: title, subtitle, content, tag,
              instance, author, blog, lang, license;
              date: before, after);

    let str_query = parsed_query.to_string();

    if str_query.is_empty() {
        render!(search::index(
            &rockets.to_context(),
            &format!("{}", Utc::today().format("%Y-%m-d"))
        ))
    } else {
        let res = rockets
            .searcher
            .search_document(&conn, parsed_query, page.limits());
        let next_page = if res.is_empty() { 0 } else { page.0 + 1 };
        render!(search::result(
            &rockets.to_context(),
            &str_query,
            res,
            page.0,
            next_page
        ))
    }
}

#[get("/search?<query..>")]
pub fn search(query: Option<Form<SearchQuery>>, rockets: PlumeRocket) -> Ructe {
    search_guts(query, rockets)
}

pub mod custom {
    use plume_models::PlumeRocket;
    use rocket::request::Form;
    use template_utils::Ructe;

    #[get("/<_custom_domain>/search?<query..>")]
    pub fn search(
        _custom_domain: String,
        query: Option<Form<super::SearchQuery>>,
        rockets: PlumeRocket,
    ) -> Ructe {
        super::search_guts(query, rockets)
    }
}
