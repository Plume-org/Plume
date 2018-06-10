use rocket::{request::Form, response::Redirect};
use rocket_contrib::Template;
use serde_json;

use BASE_URL;
use activity_pub::inbox::Inbox;
use db_conn::DbConn;
use models::{
    posts::Post,
    users::User,
    instance::*
};

#[get("/")]
fn index(conn: DbConn, user: Option<User>) -> Template {
    match Instance::get_local(&*conn) {
        Some(inst) => {
            let recents = Post::get_recents(&*conn, 6);

            Template::render("instance/index", json!({
                "instance": inst,
                "account": user,
                "recents": recents.into_iter().map(|p| {
                    json!({
                        "post": p,
                        "author": ({
                            let author = &p.get_authors(&*conn)[0];
                            let mut json = serde_json::to_value(author).unwrap();
                            json["fqn"] = serde_json::Value::String(author.get_fqn(&*conn));
                            json
                        }),
                        "url": format!("/~/{}/{}/", p.get_blog(&*conn).actor_id, p.slug),
                        "date": p.creation_date.timestamp()
                    })
                }).collect::<Vec<serde_json::Value>>()
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

#[post("/inbox", data = "<data>")]
fn shared_inbox(conn: DbConn, data: String) -> String {
    let act: serde_json::Value = serde_json::from_str(&data[..]).unwrap();
    let instance = Instance::get_local(&*conn).unwrap();
    instance.received(&*conn, act);
    String::from("")
}
