use rocket::{
    FromForm,
    form::Form,
    response::{status, Flash, Redirect},
};
use rocket::serde::json::Json;
use rocket_i18n::I18n;
use scheduled_thread_pool::ScheduledThreadPool;
use std::str::FromStr;
use validator::{Validate, ValidationErrors};

use crate::inbox;
use crate::routes::{errors::ErrorPage, rocket_uri_macro_static_files, Page, RespondOrRedirect};
use crate::template_utils::{IntoContext, PostCard, Ructe};
use plume_common::activity_pub::{broadcast, inbox::FromId};
use plume_models::{
    admin::*,
    blocklisted_emails::*,
    comments::Comment,
    db_conn::DbConn,
    headers::Headers,
    instance::*,
    posts::Post,
    safe_string::SafeString,
    timeline::Timeline,
    users::{Role, User},
    Connection, Error, PlumeRocket, CONFIG,
};

#[get("/")]
pub fn index(mut conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let all_tl = Timeline::list_all_for_user(&mut conn, rockets.user.clone().map(|u| u.id))?;
    if all_tl.is_empty() {
        Err(Error::NotFound.into())
    } else {
        let inst = Instance::get_local()?;
        let page = Page::default();
        let tl = &all_tl[0];
        let total_posts = tl.count_posts(&mut conn)?;
        let user_count = User::count_local(&mut conn)?;
        let post_count = Post::count_local(&mut conn)?;

        let pages = tl.get_page(&mut conn, page.limits())?;
        let posts = PostCard::from_posts(&mut conn, pages, &rockets.user);

        Ok(render!(instance::index_html(
            &(&mut conn, &rockets).to_context(),
            inst,
            user_count,
            post_count,
            tl.id,
            posts,
            all_tl,
            Page::total(total_posts as i32)
        )))
    }
}

#[get("/admin")]
pub fn admin(_admin: InclusiveAdmin, mut conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let local_inst = Instance::get_local()?;
    Ok(render!(instance::admin_html(
        &(&mut conn, &rockets).to_context(),
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

#[get("/admin", rank = 2)]
pub fn admin_mod(_mod: Moderator, mut conn: DbConn, rockets: PlumeRocket) -> Ructe {
    render!(instance::admin_mod_html(&(&mut conn, &rockets).to_context()))
}

#[derive(Clone, FromForm, Validate)]
pub struct InstanceSettingsForm {
    #[validate(length(min = 1))]
    pub name: String,
    pub open_registrations: bool,
    pub short_description: SafeString,
    pub long_description: SafeString,
    #[validate(length(min = 1))]
    pub default_license: String,
}

#[post("/admin", data = "<form>")]
pub fn update_settings(
    _admin: Admin,
    form: Form<InstanceSettingsForm>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> RespondOrRedirect {
    if let Err(e) = form.validate() {
        let local_inst =
            Instance::get_local().expect("instance::update_settings: local instance error");
        render!(instance::admin_html(
            &(&mut conn, &rockets).to_context(),
            local_inst,
            form.clone(),
            e
        ))
        .into()
    } else {
        let instance =
            Instance::get_local().expect("instance::update_settings: local instance error");
        instance
            .update(
                &mut conn,
                form.name.clone(),
                form.open_registrations,
                form.short_description.clone(),
                form.long_description.clone(),
                form.default_license.clone(),
            )
            .expect("instance::update_settings: save error");
        Flash::success(
            Redirect::to(uri!(admin)),
            i18n!(rockets.intl.catalog, "Instance settings have been saved."),
        )
        .into()
    }
}

#[get("/admin/instances?<page>")]
pub fn admin_instances(
    _mod: Moderator,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let instances = Instance::page(&mut conn, page.limits())?;
    let page_total = Page::total(Instance::count(&mut conn)? as i32);
    Ok(render!(instance::list_html(
        &(&mut conn, &rockets).to_context(),
        Instance::get_local()?,
        instances,
        page.0,
        page_total
    )))
}

#[post("/admin/instances/<id>/block")]
pub fn toggle_block(
    _mod: Moderator,
    mut conn: DbConn,
    id: i32,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    let inst = Instance::get(&mut conn, id)?;
    let message = if inst.blocked {
        i18n!(intl.catalog, "{} has been unblocked."; &inst.name)
    } else {
        i18n!(intl.catalog, "{} has been blocked."; &inst.name)
    };

    inst.toggle_block(&mut conn)?;
    Ok(Flash::success(
        Redirect::to(uri!(admin_instances(page = _))),
        message,
    ))
}

#[get("/admin/users?<page>", rank = 2)]
pub fn admin_users(
    _mod: Moderator,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let page_total = Page::total(User::count_local(&mut conn)? as i32);

    let users = User::get_local_page(&mut conn, page.limits())?
        .into_iter()
        .map(|u| {
            let avatar_url = u.avatar_url(&mut conn);
            (u, avatar_url)
        })
        .collect();

    Ok(render!(instance::users_html(
        &(&mut conn, &rockets).to_context(),
        users,
        None,
        page.0,
        page_total
    )))
}
#[get("/admin/users?<user>&<page>", rank = 1)]
pub fn admin_search_users(
    _mod: Moderator,
    user: &str,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let page_total = Page::total(User::count_local(&mut conn)? as i32);

    let users = (if user.is_empty() {
        User::get_local_page(&mut conn, page.limits())?
    } else {
        User::search_local_by_name(&mut conn, user, page.limits())?
    }).into_iter()
        .map(|u| {
            let avatar_url = u.avatar_url(&mut conn);
            (u, avatar_url)
        })
        .collect();

    Ok(render!(instance::users_html(
        &(&mut conn, &rockets).to_context(),
        users,
        Some(user),
        page.0,
        page_total
    )))
}

#[derive(FromForm)]
pub struct BlocklistEmailDeletion {
    ids: Vec<i32>,
}
#[post("/admin/emails/delete", data = "<form>")]
pub fn delete_email_blocklist(
    _mod: Moderator,
    form: Form<BlocklistEmailDeletion>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    BlocklistedEmail::delete_entries(&mut conn, &form.ids)?;
    Ok(Flash::success(
        Redirect::to(uri!(admin_email_blocklist(page = _))),
        i18n!(rockets.intl.catalog, "Blocks deleted"),
    ))
}

#[post("/admin/emails/new", data = "<form>")]
pub fn add_email_blocklist(
    _mod: Moderator,
    form: Form<NewBlocklistedEmail>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let result = BlocklistedEmail::insert(&mut conn, form.into_inner());

    if let Err(Error::Db(_)) = result {
        Ok(Flash::error(
            Redirect::to(uri!(admin_email_blocklist(page = _))),
            i18n!(rockets.intl.catalog, "Email already blocked"),
        ))
    } else {
        Ok(Flash::success(
            Redirect::to(uri!(admin_email_blocklist(page = _))),
            i18n!(rockets.intl.catalog, "Email Blocked"),
        ))
    }
}
#[get("/admin/emails?<page>")]
pub fn admin_email_blocklist(
    _mod: Moderator,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let page_total = Page::total(User::count_local(&mut conn)? as i32);
    let block_page = BlocklistedEmail::page(&mut conn, page.limits())?;

    Ok(render!(instance::emailblocklist_html(
        &(&mut conn, &rockets).to_context(),
        block_page,
        page.0,
        page_total
    )))
}

/// A structure to handle forms that are a list of items on which actions are applied.
/// This is for instance the case of the user list in the administration.
#[derive(FromForm)]
pub struct MultiAction {
    ids: Vec<i32>,
    action: UserActions,
}

#[derive(FromFormField)]
pub enum UserActions {
    Admin,
    RevokeAdmin,
    Moderator,
    RevokeModerator,
    Ban,
}

impl FromStr for UserActions {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(UserActions::Admin),
            "un-admin" => Ok(UserActions::RevokeAdmin),
            "moderator" => Ok(UserActions::Moderator),
            "un-moderator" => Ok(UserActions::RevokeModerator),
            "ban" => Ok(UserActions::Ban),
            _ => Err(()),
        }
    }
}

#[post("/admin/users/edit", data = "<form>")]
pub async fn edit_users(
    moderator: Moderator,
    form: Form<MultiAction>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    // you can't change your own rights
    if form.ids.contains(&moderator.0.id) {
        return Ok(Flash::error(
            Redirect::to(uri!(admin_users(page = _))),
            i18n!(rockets.intl.catalog, "You can't change your own rights."),
        ));
    }

    // moderators can't grant or revoke admin rights
    if !moderator.0.is_admin() {
        match form.action {
            UserActions::Admin | UserActions::RevokeAdmin => {
                return Ok(Flash::error(
                    Redirect::to(uri!(admin_users(page = _))),
                    i18n!(
                        rockets.intl.catalog,
                        "You are not allowed to take this action."
                    ),
                ))
            }
            _ => {}
        }
    }

    let worker = &*rockets.worker;
    match form.action {
        UserActions::Admin => {
            for u in form.ids.clone() {
                User::get(&mut conn, u)?.set_role(&mut conn, Role::Admin)?;
            }
        }
        UserActions::Moderator => {
            for u in form.ids.clone() {
                User::get(&mut conn, u)?.set_role(&mut conn, Role::Moderator)?;
            }
        }
        UserActions::RevokeAdmin | UserActions::RevokeModerator => {
            for u in form.ids.clone() {
                User::get(&mut conn, u)?.set_role(&mut conn, Role::Normal)?;
            }
        }
        UserActions::Ban => {
            for u in form.ids.clone() {
                ban(u, &mut conn, worker).await?;
            }
        }
    }

    Ok(Flash::success(
        Redirect::to(uri!(admin_users(page = _))),
        i18n!(rockets.intl.catalog, "Done."),
    ))
}

async fn ban(id: i32, conn: &mut Connection, worker: &ScheduledThreadPool) -> Result<(), ErrorPage> {
    let u = User::get(conn, id)?;
    u.delete(conn).await?;
    if Instance::get_local()
        .map(|i| u.instance_id == i.id)
        .unwrap_or(false)
    {
        BlocklistedEmail::insert(
            conn,
            NewBlocklistedEmail {
                email_address: u.email.clone().unwrap(),
                note: "Banned".to_string(),
                notify_user: false,
                notification_text: "".to_owned(),
            },
        )
        .unwrap();
        let target = User::one_by_instance(conn)?;
        let delete_act = u.delete_activity(conn)?;
        worker.execute(move || broadcast(&u, delete_act, target, CONFIG.proxy().cloned()));
    }

    Ok(())
}

#[post("/inbox", data = "<data>")]
pub async fn shared_inbox(
    conn: DbConn,
    data: inbox::SignedJson<serde_json::Value>,
    headers: Headers<'_>,
) -> Result<String, status::BadRequest<&'static str>> {
    inbox::handle_incoming(conn, data, headers).await
}

#[get("/remote_interact?<target>")]
pub async fn interact(mut conn: DbConn, user: Option<User>, target: String) -> Option<Redirect> {
    if User::find_by_fqn(&mut conn, &target).await.is_ok() {
        return Some(Redirect::to(uri!(super::user::details(name = target))));
    }

    if let Ok(post) = Post::from_id(&mut conn, &target, None, CONFIG.proxy()).await {
        return Some(Redirect::to(uri!(
            super::posts::details(blog = post.get_blog(&mut conn).expect("Can't retrieve blog").fqn,
            slug = &post.slug,
            responding_to = _
        ))));
    }

    if let Ok(comment) = Comment::from_id(&mut conn, &target, None, CONFIG.proxy()).await {
        if comment.can_see(&mut conn, user.as_ref()) {
            let post = comment.get_post(&mut conn).expect("Can't retrieve post");
            return Some(Redirect::to(uri!(
                super::posts::details(blog =
                    post.get_blog(&mut conn).expect("Can't retrieve blog").fqn,
                slug = &post.slug,
                responding_to = Some(comment.id)
            ))));
        }
    }
    None
}

#[get("/nodeinfo/<version>")]
pub fn nodeinfo(mut conn: DbConn, version: String) -> Result<Json<serde_json::Value>, ErrorPage> {
    if version != "2.0" && version != "2.1" {
        return Err(ErrorPage::from(Error::NotFound));
    }

    let local_inst = Instance::get_local()?;
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
                "total": User::count_local(&mut conn)?
            },
            "localPosts": Post::count_local(&mut conn)?,
            "localComments": Comment::count_local(&mut conn)?
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
pub fn about(mut conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let admin = Instance::get_local()?.main_admin(&mut conn)?;
    let count_user = User::count_local(&mut conn)?;
    let count_post = Post::count_local(&mut conn)?;
    let instance_count = Instance::count(&mut conn)? - 1;
    let avatar_url = admin.avatar_url(&mut conn);

    Ok(render!(instance::about_html(
        &(&mut conn, &rockets).to_context(),
        Instance::get_local()?,
        admin,
        avatar_url,
        count_user,
        count_post,
        instance_count
    )))
}

#[get("/privacy")]
pub fn privacy(mut conn: DbConn, rockets: PlumeRocket) -> Ructe {
    render!(instance::privacy_html(&(&mut conn, &rockets).to_context()))
}

#[get("/manifest.json")]
pub fn web_manifest() -> Result<Json<serde_json::Value>, ErrorPage> {
    let instance = Instance::get_local()?;
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
            .map(|i| i.with_prefix(&uri!(static_files(file = "")).to_string()))
            .collect::<Vec<_>>()
    })))
}
