use rocket::{request::LenientForm, response::{status, Redirect}};
use rocket_contrib::json::Json;
use rocket_i18n::I18n;
use serde_json;
use validator::{Validate, ValidationErrors};

use plume_common::activity_pub::sign::{Signable,
    verify_http_headers};
use plume_models::{
    admin::Admin,
    comments::Comment,
    db_conn::DbConn,
    headers::Headers,
    posts::Post,
    users::User,
    safe_string::SafeString,
    instance::*
};
use inbox::Inbox;
use routes::{Page, Ructe};

#[get("/")]
pub fn index(conn: DbConn, user: Option<User>, intl: I18n) -> Ructe {
    match Instance::get_local(&*conn) {
        Some(inst) => {
            let federated = Post::get_recents_page(&*conn, Page::first().limits());
            let local = Post::get_instance_page(&*conn, inst.id, Page::first().limits());
            let user_feed = user.clone().map(|user| {
                let followed = user.get_following(&*conn);
                let mut in_feed = followed.into_iter().map(|u| u.id).collect::<Vec<i32>>();
                in_feed.push(user.id);
                Post::user_feed_page(&*conn, in_feed, Page::first().limits())
            });

            render!(instance::index(
                (&*conn, &intl.catalog, user),
                inst,
                User::count_local(&*conn) as i32,
                Post::count_local(&*conn) as i32,
                local,
                federated,
                user_feed
            ))
        }
        None => {
            render!(errors::server_error(
                (&*conn, &intl.catalog, user)
            ))
        }
    }
}

#[get("/local?<page>")]
pub fn paginated_local(conn: DbConn, user: Option<User>, page: Page, intl: I18n) -> Ructe {
    let instance = Instance::get_local(&*conn).expect("instance::paginated_local: local instance not found error");
    let articles = Post::get_instance_page(&*conn, instance.id, page.limits());
    render!(instance::local(
        (&*conn, &intl.catalog, user),
        instance,
        articles,
        page.0,
        Page::total(Post::count_local(&*conn) as i32)
    ))
}

#[get("/local")]
pub fn local(conn: DbConn, user: Option<User>, intl: I18n) -> Ructe {
    paginated_local(conn, user, Page::first(), intl)
}

#[get("/feed")]
pub fn feed(conn: DbConn, user: User, intl: I18n) -> Ructe {
    paginated_feed(conn, user, Page::first(), intl)
}

#[get("/feed?<page>")]
pub fn paginated_feed(conn: DbConn, user: User, page: Page, intl: I18n) -> Ructe {
    let followed = user.get_following(&*conn);
    let mut in_feed = followed.into_iter().map(|u| u.id).collect::<Vec<i32>>();
    in_feed.push(user.id);
    let articles = Post::user_feed_page(&*conn, in_feed, page.limits());
    render!(instance::feed(
        (&*conn, &intl.catalog, Some(user)),
        articles,
        page.0,
        Page::total(Post::count_local(&*conn) as i32)
    ))
}

#[get("/federated")]
pub fn federated(conn: DbConn, user: Option<User>, intl: I18n) -> Ructe {
    paginated_federated(conn, user, Page::first(), intl)
}

#[get("/federated?<page>")]
pub fn paginated_federated(conn: DbConn, user: Option<User>, page: Page, intl: I18n) -> Ructe {
    let articles = Post::get_recents_page(&*conn, page.limits());
    render!(instance::federated(
        (&*conn, &intl.catalog, user),
        articles,
        page.0,
        Page::total(Post::count_local(&*conn) as i32)
    ))
}

#[get("/admin")]
pub fn admin(conn: DbConn, admin: Admin, intl: I18n) -> Ructe {
    let local_inst = Instance::get_local(&*conn).expect("instance::admin: local instance not found");
    render!(instance::admin(
        (&*conn, &intl.catalog, Some(admin.0)),
        local_inst.clone(),
        InstanceSettingsForm {
            name: local_inst.name.clone(),
            open_registrations: local_inst.open_registrations,
            short_description: local_inst.short_description,
            long_description: local_inst.long_description,
            default_license: local_inst.default_license,
        },
        ValidationErrors::default()
    ))
}

#[derive(Clone, FromForm, Validate, Serialize)]
pub struct InstanceSettingsForm {
    #[validate(length(min = "1"))]
    pub name: String,
    pub open_registrations: bool,
    pub short_description: SafeString,
    pub long_description: SafeString,
    #[validate(length(min = "1"))]
    pub default_license: String
}

#[post("/admin", data = "<form>")]
pub fn update_settings(conn: DbConn, admin: Admin, form: LenientForm<InstanceSettingsForm>, intl: I18n) -> Result<Redirect, Ructe> {
    form.validate()
        .map(|_| {
            let instance = Instance::get_local(&*conn).expect("instance::update_settings: local instance not found error");
            instance.update(&*conn,
                form.name.clone(),
                form.open_registrations,
                form.short_description.clone(),
                form.long_description.clone());
            Redirect::to(uri!(admin))
        })
        .map_err(|e| {
            let local_inst = Instance::get_local(&*conn).expect("instance::update_settings: local instance not found");
            render!(instance::admin(
                (&*conn, &intl.catalog, Some(admin.0)),
                local_inst,
                form.clone(),
                e
            ))
        })
}

#[get("/admin/instances")]
pub fn admin_instances(admin: Admin, conn: DbConn, intl: I18n) -> Ructe {
    admin_instances_paginated(admin, conn, Page::first(), intl)
}

#[get("/admin/instances?<page>")]
pub fn admin_instances_paginated(admin: Admin, conn: DbConn, page: Page, intl: I18n) -> Ructe {
    let instances = Instance::page(&*conn, page.limits());
    render!(instance::list(
        (&*conn, &intl.catalog, Some(admin.0)),
        Instance::get_local(&*conn).expect("admin_instances: local instance error"),
        instances,
        page.0,
        Page::total(Instance::count(&*conn) as i32)
    ))
}

#[post("/admin/instances/<id>/block")]
pub fn toggle_block(_admin: Admin, conn: DbConn, id: i32) -> Redirect {
    if let Some(inst) = Instance::get(&*conn, id) {
        inst.toggle_block(&*conn);
    }

    Redirect::to(uri!(admin_instances))
}

#[get("/admin/users")]
pub fn admin_users(admin: Admin, conn: DbConn, intl: I18n) -> Ructe {
    admin_users_paginated(admin, conn, Page::first(), intl)
}

#[get("/admin/users?<page>")]
pub fn admin_users_paginated(admin: Admin, conn: DbConn, page: Page, intl: I18n) -> Ructe {
    render!(instance::users(
        (&*conn, &intl.catalog, Some(admin.0)),
        User::get_local_page(&*conn, page.limits()),
        page.0,
        Page::total(User::count_local(&*conn) as i32)
    ))
}

#[post("/admin/users/<id>/ban")]
pub fn ban(_admin: Admin, conn: DbConn, id: i32) -> Redirect {
    if let Some(u) = User::get(&*conn, id) {
        u.delete(&*conn);
    }
    Redirect::to(uri!(admin_users))
}

#[post("/inbox", data = "<data>")]
pub fn shared_inbox(conn: DbConn, data: String, headers: Headers) -> Result<String, status::BadRequest<&'static str>> {
    let act: serde_json::Value = serde_json::from_str(&data[..]).expect("instance::shared_inbox: deserialization error");

    let activity = act.clone();
    let actor_id = activity["actor"].as_str()
        .or_else(|| activity["actor"]["id"].as_str()).ok_or(status::BadRequest(Some("Missing actor id for activity")))?;

    let actor = User::from_url(&conn, actor_id).expect("instance::shared_inbox: user error");
    if !verify_http_headers(&actor, &headers.0, &data).is_secure() &&
        !act.clone().verify(&actor) {
        println!("Rejected invalid activity supposedly from {}, with headers {:?}", actor.username, headers.0);
        return Err(status::BadRequest(Some("Invalid signature")));
    }

    if Instance::is_blocked(&*conn, actor_id) {
        return Ok(String::new());
    }
    let instance = Instance::get_local(&*conn).expect("instance::shared_inbox: local instance not found error");
    Ok(match instance.received(&*conn, act) {
        Ok(_) => String::new(),
        Err(e) => {
            println!("Shared inbox error: {}\n{}", e.as_fail(), e.backtrace());
            format!("Error: {}", e.as_fail())
        }
    })
}

#[get("/nodeinfo")]
pub fn nodeinfo(conn: DbConn) -> Json<serde_json::Value> {
    Json(json!({
        "version": "2.0",
        "software": {
            "name": "Plume",
            "version": env!("CARGO_PKG_VERSION")
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
pub fn about(user: Option<User>, conn: DbConn, intl: I18n) -> Ructe {
    render!(instance::about(
        (&*conn, &intl.catalog, user),
        Instance::get_local(&*conn).expect("Local instance not found"),
        Instance::get_local(&*conn).expect("Local instance not found").main_admin(&*conn),
        User::count_local(&*conn),
        Post::count_local(&*conn),
        Instance::count(&*conn) - 1
    ))
}

#[get("/manifest.json")]
pub fn web_manifest(conn: DbConn) -> Json<serde_json::Value> {
    let instance = Instance::get_local(&*conn).expect("instance::web_manifest: local instance not found error");
    Json(json!({
        "name": &instance.name,
        "description": &instance.short_description,
        "start_url": String::from("/"),
        "scope": String::from("/"),
        "display": String::from("standalone"),
        "background_color": String::from("#f4f4f4"),
        "theme_color": String::from("#7765e3"),
        "icons": [
            {
                "src": "/static/icons/trwnh/feather/plumeFeather48.png",
                "sizes": "48x48",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather72.png",
                "sizes": "72x72",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather96.png",
                "sizes": "96x96",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather144.png",
                "sizes": "144x144",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather160.png",
                "sizes": "160x160",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather192.png",
                "sizes": "192x192",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather256.png",
                "sizes": "256x256",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather512.png",
                "sizes": "512x512",
                "type": "image/png"
            },
            {
                "src": "/static/icons/trwnh/feather/plumeFeather.svg"
            }
        ]
    }))
}
