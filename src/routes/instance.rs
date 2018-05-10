use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;
use std::collections::HashMap;

use BASE_URL;
use db_conn::DbConn;
use models::users::User;
use models::instance::*;

#[get("/")]
fn index(conn: DbConn, user: Option<User>) -> Template {
    match Instance::get_local(&*conn) {
        Some(inst) => {
            Template::render("instance/index", json!({
                "instance": inst,
                "account": user
            }))
        }
        None => {
            Template::render("errors/500", json!({
                "error_message": "You need to configure your instance before using it."
            }))
        }
    }
}

#[get("/configure")]
fn configure() -> Template {
    Template::render("instance/configure", json!({}))
}

#[derive(FromForm)]
struct NewInstanceForm {
    name: String
}

#[post("/configure", data = "<data>")]
fn post_config(conn: DbConn, data: Form<NewInstanceForm>) -> Redirect {
    let form = data.get();
    let inst = Instance::insert(
        &*conn,
        BASE_URL.as_str().to_string(),
        form.name.to_string(),
        true);
    if inst.has_admin(&*conn) {
        Redirect::to("/")
    } else {
        Redirect::to("/users/new")
    }
}
