use rocket::{request::LenientForm, response::{status, Redirect}};
use rocket_contrib::json::Json;
use rocket_i18n::I18n;
use serde_json;
use validator::{Validate, ValidationErrors};

use plume_models::{
    Error,
    admin::Admin,
    comments::Comment,
    db_conn::DbConn,
    headers::Headers,
    posts::Post,
    users::User,
    safe_string::SafeString,
    instance::*
};
use inbox;
use routes::{Page, errors::ErrorPage};
use template_utils::Ructe;
use Searcher;

#[get("/")]
pub fn index(conn: DbConn, user: Option<User>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let inst = Instance::get_local(&*conn)?;
    let federated = Post::get_recents_page(&*conn, Page::default().limits())?;
    let local = Post::get_instance_page(&*conn, inst.id, Page::default().limits())?;
    let user_feed = user.clone().and_then(|user| {
        let followed = user.get_followed(&*conn).ok()?;
        let mut in_feed = followed.into_iter().map(|u| u.id).collect::<Vec<i32>>();
        in_feed.push(user.id);
        Post::user_feed_page(&*conn, in_feed, Page::default().limits()).ok()
    });

    Ok(render!(instance::index(
        &(&*conn, &intl.catalog, user),
        inst,
        User::count_local(&*conn)?,
        Post::count_local(&*conn)?,
        local,
        federated,
        user_feed
    )))
}

#[get("/local?<page>")]
pub fn local(conn: DbConn, user: Option<User>, page: Option<Page>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let instance = Instance::get_local(&*conn)?;
    let articles = Post::get_instance_page(&*conn, instance.id, page.limits())?;
    Ok(render!(instance::local(
        &(&*conn, &intl.catalog, user),
        instance,
        articles,
        page.0,
        Page::total(Post::count_local(&*conn)? as i32)
    )))
}

#[get("/feed?<page>")]
pub fn feed(conn: DbConn, user: User, page: Option<Page>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let followed = user.get_followed(&*conn)?;
    let mut in_feed = followed.into_iter().map(|u| u.id).collect::<Vec<i32>>();
    in_feed.push(user.id);
    let articles = Post::user_feed_page(&*conn, in_feed, page.limits())?;
    Ok(render!(instance::feed(
        &(&*conn, &intl.catalog, Some(user)),
        articles,
        page.0,
        Page::total(Post::count_local(&*conn)? as i32)
    )))
}

#[get("/federated?<page>")]
pub fn federated(conn: DbConn, user: Option<User>, page: Option<Page>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let articles = Post::get_recents_page(&*conn, page.limits())?;
    Ok(render!(instance::federated(
        &(&*conn, &intl.catalog, user),
        articles,
        page.0,
        Page::total(Post::count_local(&*conn)? as i32)
    )))
}

#[get("/admin")]
pub fn admin(conn: DbConn, admin: Admin, intl: I18n) -> Result<Ructe, ErrorPage> {
    let local_inst = Instance::get_local(&*conn)?;
    Ok(render!(instance::admin(
        &(&*conn, &intl.catalog, Some(admin.0)),
        local_inst.clone(),
        InstanceSettingsForm {
            name: local_inst.name.clone(),
            open_registrations: local_inst.open_registrations,
            short_description: local_inst.short_description,
            long_description: local_inst.long_description,
            default_license: local_inst.default_license,
        },
        ValidationErrors::default()
    )))
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
        .and_then(|_| {
            let instance = Instance::get_local(&*conn).expect("instance::update_settings: local instance error");
            instance.update(&*conn,
                form.name.clone(),
                form.open_registrations,
                form.short_description.clone(),
                form.long_description.clone()).expect("instance::update_settings: save error");
            Ok(Redirect::to(uri!(admin)))
        })
       .or_else(|e| {
            let local_inst = Instance::get_local(&*conn).expect("instance::update_settings: local instance error");
            Err(render!(instance::admin(
                &(&*conn, &intl.catalog, Some(admin.0)),
                local_inst,
                form.clone(),
                e
            )))
        })
}

#[get("/admin/instances?<page>")]
pub fn admin_instances(admin: Admin, conn: DbConn, page: Option<Page>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let instances = Instance::page(&*conn, page.limits())?;
    Ok(render!(instance::list(
        &(&*conn, &intl.catalog, Some(admin.0)),
        Instance::get_local(&*conn)?,
        instances,
        page.0,
        Page::total(Instance::count(&*conn)? as i32)
    )))
}

#[post("/admin/instances/<id>/block")]
pub fn toggle_block(_admin: Admin, conn: DbConn, id: i32) -> Result<Redirect, ErrorPage> {
    if let Ok(inst) = Instance::get(&*conn, id) {
        inst.toggle_block(&*conn)?;
    }

    Ok(Redirect::to(uri!(admin_instances: page = _)))
}

#[get("/admin/users?<page>")]
pub fn admin_users(admin: Admin, conn: DbConn, page: Option<Page>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    Ok(render!(instance::users(
        &(&*conn, &intl.catalog, Some(admin.0)),
        User::get_local_page(&*conn, page.limits())?,
        page.0,
        Page::total(User::count_local(&*conn)? as i32)
    )))
}

#[post("/admin/users/<id>/ban")]
pub fn ban(_admin: Admin, conn: DbConn, id: i32, searcher: Searcher) -> Result<Redirect, ErrorPage> {
    if let Ok(u) = User::get(&*conn, id) {
        u.delete(&*conn, &searcher)?;
    }
    Ok(Redirect::to(uri!(admin_users: page = _)))
}

#[post("/inbox", data = "<data>")]
pub fn shared_inbox(
    conn: DbConn,
    data: inbox::SignedJson<serde_json::Value>,
    headers: Headers,
    searcher: Searcher
) -> Result<String, status::BadRequest<&'static str>> {
    inbox::handle_incoming(conn, data, headers, searcher)
}

#[get("/nodeinfo/<version>")]
pub fn nodeinfo(conn: DbConn, version: String) -> Result<Json<serde_json::Value>, ErrorPage> {
    if version != "2.0" || version != "2.1" {
        return Err(ErrorPage::from(Error::NotFound));
    }

    let local_inst = Instance::get_local(&*conn)?;
    let mut doc = json!({
        "version": version,
        "software": {
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
        },
        "protocols": ["activitypub"],
        "services": {
            "inbound": [],
            "outbound": []
        },
        "openRegistrations": local_inst.open_registrations,
        "usage": {
            "users": {
                "total": User::count_local(&*conn)?
            },
            "localPosts": Post::count_local(&*conn)?,
            "localComments": Comment::count_local(&*conn)?
        },
        "metadata": {
            "nodeName": local_inst.name,
            "nodeDescription": local_inst.short_description
        }
    });

    if version == "2.1" {
        doc["software"]["repository"] = json!(env!("CARGO_PKG_REPOSITORY"));
    }

    Ok(Json(doc))
}

#[get("/about")]
pub fn about(user: Option<User>, conn: DbConn, intl: I18n) -> Result<Ructe, ErrorPage> {
    Ok(render!(instance::about(
        &(&*conn, &intl.catalog, user),
        Instance::get_local(&*conn)?,
        Instance::get_local(&*conn)?.main_admin(&*conn)?,
        User::count_local(&*conn)?,
        Post::count_local(&*conn)?,
        Instance::count(&*conn)? - 1
    )))
}

#[get("/manifest.json")]
pub fn web_manifest(conn: DbConn) -> Result<Json<serde_json::Value>, ErrorPage> {
    let instance = Instance::get_local(&*conn)?;
    Ok(Json(json!({
        "name": &instance.name,
        "description": &instance.short_description,
        "start_url": String::from("/"),
        "scope": String::from("/"),
        "display": String::from("standalone"),
        "background_color": String::from("#f4f4f4"),
        "theme_color": String::from("#7765e3"),
        "categories": [String::from("social")],
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
    })))
}
