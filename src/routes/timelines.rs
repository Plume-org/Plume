#![allow(dead_code)]

use crate::{routes::errors::ErrorPage, template_utils::Ructe};
use plume_models::PlumeRocket;
use rocket::response::Redirect;

// TODO

#[get("/timeline/<_id>")]
pub fn details(_id: i32, _rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    unimplemented!()
}

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
