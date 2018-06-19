use gettextrs::gettext;
use rocket_contrib::{Json, Template};
use serde_json;

use activity_pub::inbox::Inbox;
use db_conn::DbConn;
use models::{
    comments::Comment,
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
                "recents": recents.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
            }))
        }
        None => {
            Template::render("errors/500", json!({
                "error_message": gettext("You need to configure your instance before using it.".to_string())
            }))
        }
    }
}

#[post("/inbox", data = "<data>")]
fn shared_inbox(conn: DbConn, data: String) -> String {
    let act: serde_json::Value = serde_json::from_str(&data[..]).unwrap();
    let instance = Instance::get_local(&*conn).unwrap();
    instance.received(&*conn, act);
    String::from("")
}

#[get("/nodeinfo")]
fn nodeinfo(conn: DbConn) -> Json<serde_json::Value> {
    Json(json!({
        "version": "2.0",
        "software": {
            "name": "Plume",
            "version": "0.1.0"
        },
        "protocols": ["activitypub"],
        "services": {
            "inbound": [],
            "outbound": []
        },
        "openRegistrations": true,
        "usage": {
            "users": {
                "total": User::count_local(&*conn)
            },
            "localPosts": Post::count_local(&*conn),
            "localComments": Comment::count_local(&*conn)
        },
        "metadata": {}
    }))
}
