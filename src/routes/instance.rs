use rocket::{
    request::LenientForm,
    response::{status, Flash, Redirect},
};
use rocket_contrib::json::Json;
use rocket_i18n::I18n;
use serde_json;
use validator::{Validate, ValidationErrors};

use inbox;
use plume_common::activity_pub::{broadcast, inbox::FromId};
use plume_models::{
    admin::Admin, comments::Comment, db_conn::DbConn, headers::Headers, instance::*, posts::Post,
    safe_string::SafeString, users::User, Error, PlumeRocket, CONFIG,
};
use routes::{errors::ErrorPage, rocket_uri_macro_static_files, Page};
use template_utils::{IntoContext, Ructe};

#[get("/")]
pub fn index(rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let inst = Instance::get_local(conn)?;
    let federated = Post::get_recents_page(conn, Page::default().limits())?;
    let local = Post::get_instance_page(conn, inst.id, Page::default().limits())?;
    let user_feed = rockets.user.clone().and_then(|user| {
        let followed = user.get_followed(conn).ok()?;
        let mut in_feed = followed.into_iter().map(|u| u.id).collect::<Vec<i32>>();
        in_feed.push(user.id);
        Post::user_feed_page(conn, in_feed, Page::default().limits()).ok()
    });

    Ok(render!(instance::index(
        &rockets.to_context(),
        inst,
        User::count_local(conn)?,
        Post::count_local(conn)?,
        local,
        federated,
        user_feed
    )))
}

#[get("/local?<page>")]
pub fn local(page: Option<Page>, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let instance = Instance::get_local(&*rockets.conn)?;
    let articles = Post::get_instance_page(&*rockets.conn, instance.id, page.limits())?;
    Ok(render!(instance::local(
        &rockets.to_context(),
        instance,
        articles,
        page.0,
        Page::total(Post::count_local(&*rockets.conn)? as i32)
    )))
}

#[get("/feed?<page>")]
pub fn feed(user: User, page: Option<Page>, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let followed = user.get_followed(&*rockets.conn)?;
    let mut in_feed = followed.into_iter().map(|u| u.id).collect::<Vec<i32>>();
    in_feed.push(user.id);
    let articles = Post::user_feed_page(&*rockets.conn, in_feed, page.limits())?;
    Ok(render!(instance::feed(
        &rockets.to_context(),
        articles,
        page.0,
        Page::total(Post::count_local(&*rockets.conn)? as i32)
    )))
}

#[get("/federated?<page>")]
pub fn federated(page: Option<Page>, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let articles = Post::get_recents_page(&*rockets.conn, page.limits())?;
    Ok(render!(instance::federated(
        &rockets.to_context(),
        articles,
        page.0,
        Page::total(Post::count_local(&*rockets.conn)? as i32)
    )))
}

#[get("/admin")]
pub fn admin(_admin: Admin, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let local_inst = Instance::get_local(&*rockets.conn)?;
    Ok(render!(instance::admin(
        &rockets.to_context(),
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

#[derive(Clone, FromForm, Validate)]
pub struct InstanceSettingsForm {
    #[validate(length(min = "1"))]
    pub name: String,
    pub open_registrations: bool,
    pub short_description: SafeString,
    pub long_description: SafeString,
    #[validate(length(min = "1"))]
    pub default_license: String,
}

#[post("/admin", data = "<form>")]
pub fn update_settings(
    _admin: Admin,
    form: LenientForm<InstanceSettingsForm>,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, Ructe> {
    let conn = &*rockets.conn;
    form.validate()
        .and_then(|_| {
            let instance =
                Instance::get_local(conn).expect("instance::update_settings: local instance error");
            instance
                .update(
                    conn,
                    form.name.clone(),
                    form.open_registrations,
                    form.short_description.clone(),
                    form.long_description.clone(),
                )
                .expect("instance::update_settings: save error");
            Ok(Flash::success(
                Redirect::to(uri!(admin)),
                i18n!(rockets.intl.catalog, "Instance settings have been saved."),
            ))
        })
        .or_else(|e| {
            let local_inst =
                Instance::get_local(conn).expect("instance::update_settings: local instance error");
            Err(render!(instance::admin(
                &rockets.to_context(),
                local_inst,
                form.clone(),
                e
            )))
        })
}

#[get("/admin/instances?<page>")]
pub fn admin_instances(
    _admin: Admin,
    page: Option<Page>,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let instances = Instance::page(&*rockets.conn, page.limits())?;
    Ok(render!(instance::list(
        &rockets.to_context(),
        Instance::get_local(&*rockets.conn)?,
        instances,
        page.0,
        Page::total(Instance::count(&*rockets.conn)? as i32)
    )))
}

#[post("/admin/instances/<id>/block")]
pub fn toggle_block(
    _admin: Admin,
    conn: DbConn,
    id: i32,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    let inst = Instance::get(&*conn, id)?;
    let message = if inst.blocked {
        i18n!(intl.catalog, "{} have been unblocked."; &inst.name)
    } else {
        i18n!(intl.catalog, "{} have been blocked."; &inst.name)
    };

    inst.toggle_block(&*conn)?;
    Ok(Flash::success(
        Redirect::to(uri!(admin_instances: page = _)),
        message,
    ))
}

#[get("/admin/users?<page>")]
pub fn admin_users(
    _admin: Admin,
    page: Option<Page>,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    Ok(render!(instance::users(
        &rockets.to_context(),
        User::get_local_page(&*rockets.conn, page.limits())?,
        page.0,
        Page::total(User::count_local(&*rockets.conn)? as i32)
    )))
}

#[post("/admin/users/<id>/ban")]
pub fn ban(_admin: Admin, id: i32, rockets: PlumeRocket) -> Result<Flash<Redirect>, ErrorPage> {
    let u = User::get(&*rockets.conn, id)?;
    u.delete(&*rockets.conn, &rockets.searcher)?;

    if Instance::get_local(&*rockets.conn)
        .map(|i| u.instance_id == i.id)
        .unwrap_or(false)
    {
        let target = User::one_by_instance(&*rockets.conn)?;
        let delete_act = u.delete_activity(&*rockets.conn)?;
        let u_clone = u.clone();
        rockets
            .worker
            .execute(move || broadcast(&u_clone, delete_act, target));
    }

    Ok(Flash::success(
        Redirect::to(uri!(admin_users: page = _)),
        i18n!(rockets.intl.catalog, "{} have been banned."; u.name()),
    ))
}

#[post("/inbox", data = "<data>")]
pub fn shared_inbox(
    rockets: PlumeRocket,
    data: inbox::SignedJson<serde_json::Value>,
    headers: Headers,
) -> Result<String, status::BadRequest<&'static str>> {
    inbox::handle_incoming(rockets, data, headers)
}

#[get("/remote_interact?<target>")]
pub fn interact(rockets: PlumeRocket, user: Option<User>, target: String) -> Option<Redirect> {
    if User::find_by_fqn(&rockets, &target).is_ok() {
        return Some(Redirect::to(uri!(super::user::details: name = target)));
    }

    if let Ok(post) = Post::from_id(&rockets, &target, None) {
        return Some(Redirect::to(
            uri!(super::posts::details: blog = post.get_blog(&rockets.conn).expect("Can't retrieve blog").fqn, slug = &post.slug, responding_to = _),
        ));
    }

    if let Ok(comment) = Comment::from_id(&rockets, &target, None) {
        if comment.can_see(&rockets.conn, user.as_ref()) {
            let post = comment
                .get_post(&rockets.conn)
                .expect("Can't retrieve post");
            return Some(Redirect::to(uri!(
                super::posts::details: blog = post
                    .get_blog(&rockets.conn)
                    .expect("Can't retrieve blog")
                    .fqn,
                slug = &post.slug,
                responding_to = comment.id
            )));
        }
    }
    None
}

#[get("/nodeinfo/<version>")]
pub fn nodeinfo(conn: DbConn, version: String) -> Result<Json<serde_json::Value>, ErrorPage> {
    if version != "2.0" && version != "2.1" {
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
pub fn about(rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    Ok(render!(instance::about(
        &rockets.to_context(),
        Instance::get_local(conn)?,
        Instance::get_local(conn)?.main_admin(conn)?,
        User::count_local(conn)?,
        Post::count_local(conn)?,
        Instance::count(conn)? - 1
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
        "icons": CONFIG.logo.other.iter()
            .map(|i| i.with_prefix(&uri!(static_files: file = "").to_string()))
            .collect::<Vec<_>>()
    })))
}
