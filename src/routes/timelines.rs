#![allow(dead_code)]

use crate::routes::Page;
use crate::template_utils::IntoContext;
use crate::{routes::errors::ErrorPage, template_utils::Ructe};
use plume_models::{db_conn::DbConn, timeline::*, PlumeRocket};
use rocket::response::Redirect;

#[get("/timeline/<id>?<page>")]
pub fn details(
    id: i32,
    conn: DbConn,
    rockets: PlumeRocket,
    page: Option<Page>,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let all_tl = Timeline::list_all_for_user(&conn, rockets.user.clone().map(|u| u.id))?;
    let tl = Timeline::get(&conn, id)?;
    let posts = tl.get_page(&conn, page.limits())?;
    let total_posts = tl.count_posts(&conn)?;
    Ok(render!(timelines::details(
        &(&conn, &rockets).to_context(),
        tl,
        posts,
        all_tl,
        page.0,
        Page::total(total_posts as i32)
    )))
}

// TODO

#[get("/timeline/new")]
pub fn new() -> Result<Ructe, ErrorPage> {
    unimplemented!()
}

#[post("/timeline/new")]
pub fn create() -> Result<Redirect, Ructe> {
    unimplemented!()
}

#[get("/timeline/<_id>/edit")]
pub fn edit(_id: i32) -> Result<Ructe, ErrorPage> {
    unimplemented!()
}

#[post("/timeline/<_id>/edit")]
pub fn update(_id: i32) -> Result<Redirect, Ructe> {
    unimplemented!()
}

#[post("/timeline/<_id>/delete")]
pub fn delete(_id: i32) -> Result<Redirect, ErrorPage> {
    unimplemented!()
}
