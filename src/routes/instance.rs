use gettextrs::gettext;
use rocket::{request::LenientForm, response::Redirect};
use rocket_contrib::{Json, Template};
use serde_json;
use validator::{Validate};

use plume_models::{
    admin::Admin,
    comments::Comment,
    db_conn::DbConn,
    posts::Post,
    users::User,
    safe_string::SafeString,
    instance::*

};
use inbox::Inbox;
use routes::Page;

#[get("/")]
fn index(conn: DbConn, user: Option<User>) -> Template {
    match Instance::get_local(&*conn) {
        Some(inst) => {
            let federated = Post::get_recents_page(&*conn, Page::first().limits());
            let local = Post::get_instance_page(&*conn, inst.id, Page::first().limits());
            let user_feed = user.clone().map(|user| {
                let followed = user.get_following(&*conn);
                Post::user_feed_page(&*conn, followed.into_iter().map(|u| u.id).collect(), Page::first().limits())
            });

            Template::render("instance/index", json!({
                "instance": inst,
                "account": user.map(|u| u.to_json(&*conn)),
                "federated": federated.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
                "local": local.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
                "user_feed": user_feed.map(|f| f.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()),
                "n_users": User::count_local(&*conn),
                "n_articles": Post::count_local(&*conn)
            }))
        }
        None => {
            Template::render("errors/500", json!({
                "error_message": gettext("You need to configure your instance before using it.".to_string())
            }))
        }
    }
}

#[get("/local?<page>")]
fn paginated_local(conn: DbConn, user: Option<User>, page: Page) -> Template {
    let instance = Instance::get_local(&*conn).unwrap();
    let articles = Post::get_instance_page(&*conn, instance.id, page.limits());
    Template::render("instance/local", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "instance": instance,
        "page": page.page,
        "n_pages": Page::total(Post::count_local(&*conn) as i32),
        "articles": articles.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/local")]
fn local(conn: DbConn, user: Option<User>) -> Template {
    paginated_local(conn, user, Page::first())
}

#[get("/feed")]
fn feed(conn: DbConn, user: User) -> Template {
    paginated_feed(conn, user, Page::first())
}

#[get("/feed?<page>")]
fn paginated_feed(conn: DbConn, user: User, page: Page) -> Template {
    let followed = user.get_following(&*conn);
    let articles = Post::user_feed_page(&*conn, followed.into_iter().map(|u| u.id).collect(), page.limits());
    Template::render("instance/feed", json!({
        "account": user.to_json(&*conn),
        "page": page.page,
        "n_pages": Page::total(Post::count_local(&*conn) as i32),
        "articles": articles.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/federated")]
fn federated(conn: DbConn, user: Option<User>) -> Template {
    paginated_federated(conn, user, Page::first())
}

#[get("/federated?<page>")]
fn paginated_federated(conn: DbConn, user: Option<User>, page: Page) -> Template {
    let articles = Post::get_recents_page(&*conn, page.limits());
    Template::render("instance/federated", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "page": page.page,
        "n_pages": Page::total(Post::count_local(&*conn) as i32),
        "articles": articles.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/admin")]
fn admin(conn: DbConn, admin: Admin) -> Template {
    Template::render("instance/admin", json!({
        "account": admin.0.to_json(&*conn),
        "instance": Instance::get_local(&*conn),
        "errors": null,
        "form": null
    }))
}

#[derive(FromForm, Validate, Serialize)]
struct InstanceSettingsForm {
    #[validate(length(min = "1"))]
    name: String,
    open_registrations: bool,
    short_description: SafeString,
    long_description: SafeString,
    #[validate(length(min = "1"))]
    default_license: String
}

#[post("/admin", data = "<form>")]
fn update_settings(conn: DbConn, admin: Admin, form: LenientForm<InstanceSettingsForm>) -> Result<Redirect, Template> {
    let form = form.get();
    form.validate()
        .map(|_| {
            let instance = Instance::get_local(&*conn).unwrap();
            instance.update(&*conn,
                form.name.clone(),
                form.open_registrations,
                form.short_description.clone(),
                form.long_description.clone());
            Redirect::to(uri!(admin))
        })
        .map_err(|e| Template::render("instance/admin", json!({
            "account": admin.0.to_json(&*conn),
            "instance": Instance::get_local(&*conn),
            "errors": e.inner(),
            "form": form
        })))
}

#[get("/admin/instances")]
fn admin_instances(admin: Admin, conn: DbConn) -> Template {
    admin_instances_paginated(admin, conn, Page::first())
}

#[get("/admin/instances?<page>")]
fn admin_instances_paginated(admin: Admin, conn: DbConn, page: Page) -> Template {
    let instances = Instance::page(&*conn, page.limits());
    Template::render("instance/list", json!({
        "account": admin.0.to_json(&*conn),
        "instances": instances,
        "instance": Instance::get_local(&*conn),
        "page": page.page,
        "n_pages": Page::total(Instance::count(&*conn) as i32),
    }))
}

#[get("/admin/instances/<id>/block")]
fn toggle_block(_admin: Admin, conn: DbConn, id: i32) -> Redirect {
    if let Some(inst) = Instance::get(&*conn, id) {
        inst.toggle_block(&*conn);
    }

    Redirect::to(uri!(admin_instances))
}

#[get("/admin/users")]
fn admin_users(admin: Admin, conn: DbConn) -> Template {
    admin_users_paginated(admin, conn, Page::first())
}

#[get("/admin/users?<page>")]
fn admin_users_paginated(admin: Admin, conn: DbConn, page: Page) -> Template {
    let users = User::get_local_page(&*conn, page.limits()).into_iter()
        .map(|u| u.to_json(&*conn)).collect::<Vec<serde_json::Value>>();

    Template::render("instance/users", json!({
        "account": admin.0.to_json(&*conn),
        "users": users,
        "page": page.page,
        "n_pages": Page::total(User::count_local(&*conn) as i32)
    }))
}

#[get("/admin/users/<id>/ban")]
fn ban(_admin: Admin, conn: DbConn, id: i32) -> Redirect {
    User::get(&*conn, id).map(|u| u.delete(&*conn));
    Redirect::to(uri!(admin_users))
}

#[post("/inbox", data = "<data>")]
fn shared_inbox(conn: DbConn, data: String) -> String {
    let act: serde_json::Value = serde_json::from_str(&data[..]).unwrap();

    let activity = act.clone();
    let actor_id = activity["actor"].as_str()
        .unwrap_or_else(|| activity["actor"]["id"].as_str().expect("No actor ID for incoming activity, blocks by panicking"));
    if Instance::is_blocked(&*conn, actor_id.to_string()) {
        return String::new();
    }
    let instance = Instance::get_local(&*conn).unwrap();
    match instance.received(&*conn, act) {
        Ok(_) => String::new(),
        Err(e) => {
            println!("Shared inbox error: {}\n{}", e.as_fail(), e.backtrace());
            format!("Error: {}", e.as_fail())
        }
    }
}

#[get("/nodeinfo")]
fn nodeinfo(conn: DbConn) -> Json<serde_json::Value> {
    Json(json!({
        "version": "2.0",
        "software": {
            "name": "Plume",
            "version": "0.2.0"
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

#[get("/about")]
fn about(user: Option<User>, conn: DbConn) -> Template {
    Template::render("instance/about", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "instance": Instance::get_local(&*conn),
        "admin": Instance::get_local(&*conn).map(|i| i.main_admin(&*conn).to_json(&*conn)),
        "version": "0.2.0",
        "n_users": User::count_local(&*conn),
        "n_articles": Post::count_local(&*conn),
        "n_instances": Instance::count(&*conn) - 1
    }))
}

#[get("/manifest.json")]
fn web_manifest(conn: DbConn) -> Json<serde_json::Value> {
    let instance = Instance::get_local(&*conn).unwrap();
    Json(json!({
        "name": &instance.name,
        "description": &instance.short_description,
        "start_url": String::from("/"),
        "scope": String::from("/"),
        "display": String::from("standalone"),
        "background_color": String::from("#f4f4f4"),
        "theme_color": String::from("#7765e3")
    }))
}
