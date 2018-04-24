use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;
use std::collections::HashMap;

use activity_pub::Activity;
use activity_pub::actor::Actor;
use db_conn::DbConn;
use models::instance::Instance;
use models::users::*;

#[get("/me")]
fn me(user: User) -> String {
    format!("Logged in as {}", user.username.to_string())
}

#[get("/@/<name>", rank = 2)]
fn details(name: String) -> String {
    format!("Hello, @{}", name)
}

#[get("/@/<name>", format = "application/activity+json", rank = 1)]
fn activity_details(name: String, conn: DbConn) -> Activity {
    let user = User::find_by_name(&*conn, name).unwrap();
    user.as_activity_pub(&*conn)
}

#[get("/users/new")]
fn new() -> Template {
    Template::render("users/new", HashMap::<String, i32>::new())
}

#[derive(FromForm)]
struct NewUserForm {
    username: String,
    email: String,
    password: String,
    password_confirmation: String
}

#[post("/users/new", data = "<data>")]
fn create(conn: DbConn, data: Form<NewUserForm>) -> Redirect {
    let inst = Instance::get_local(&*conn).unwrap();
    let form = data.get();

    if form.password == form.password_confirmation {
        User::insert(&*conn, NewUser::new_local(
            form.username.to_string(),
            form.username.to_string(),
            !inst.has_admin(&*conn),
            String::from(""),
            form.email.to_string(),
            User::hash_pass(form.password.to_string()),
            inst.id
        )).update_boxes(&*conn);
    }
    
    Redirect::to(format!("/@/{}", data.get().username).as_str())
}
