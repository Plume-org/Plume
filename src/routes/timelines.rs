use plume_models::PlumeRocket;
use crate::{template_utils::Ructe, routes::errors::ErrorPage};

#[get("/timeline/<id>")]
pub fn details(id: i32, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {

}

#[get("/timeline/new")]
pub fn new() -> Result<Ructe, ErrorPage> {

}

#[post("/timeline/new")]
pub fn create() -> Result<Redirect, Ructe> {

}

#[get("/timeline/<id>/edit")]
pub fn edit() -> Result<Ructe, ErrorPage> {

}

#[post("/timeline/<id>/edit")]
pub fn update() -> Result<Redirect, Ructe> {

}

#[post("/timeline/<id>/delete")]
pub fn delete() -> Result<Redirect, ErrorPage> {

}
