use rocket_contrib::Template;
use rocket::response::Redirect;
use rocket::request::Form;
use std::collections::HashMap;

use db_conn::DbConn;
use models::instance::*;

#[get("/configure")]
fn configure() -> Template {
    Template::render("instance/configure", HashMap::<String, i32>::new())
}

#[derive(FromForm)]
struct NewInstanceForm {
    local_domain: String,
    public_domain: String,
    name: String
}

#[post("/configure", data = "<data>")]
fn post_config(conn: DbConn, data: Form<NewInstanceForm>) -> Redirect {
    let form = data.get();
    let inst = Instance::insert(
        &*conn,
        form.local_domain.to_string(),
        form.public_domain.to_string(), 
        form.name.to_string(),
        true);
    if inst.has_admin(&*conn) {
        Redirect::to("/")
    } else {
        Redirect::to("/users/new")
    }
}
