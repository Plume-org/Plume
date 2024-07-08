use rocket::{
    request::{Form, FormItems, FromForm, LenientForm},
    response::{status, Flash, Redirect},
};
use rocket_contrib::json::Json;
use rocket_i18n::I18n;
use scheduled_thread_pool::ScheduledThreadPool;
use std::str::FromStr;
use validator::{Validate, ValidationErrors};

use crate::inbox;
use crate::routes::{errors::ErrorPage, rocket_uri_macro_static_files, Page, RespondOrRedirect};
use crate::template_utils::{IntoContext, Ructe};
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
pub fn index(conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let all_tl = Timeline::list_all_for_user(&conn, rockets.user.clone().map(|u| u.id))?;
    if all_tl.is_empty() {
        Err(Error::NotFound.into())
    } else {
        let inst = Instance::get_local()?;
        let page = Page::default();
        let tl = &all_tl[0];
        let posts = tl.get_page(&conn, page.limits())?;
        let total_posts = tl.count_posts(&conn)?;
        Ok(render!(instance::index(
            &(&conn, &rockets).to_context(),
            inst,
            User::count_local(&conn)?,
            Post::count_local(&conn)?,
            tl.id,
            posts,
            all_tl,
            Page::total(total_posts as i32)
        )))
    }
}

#[get("/admin")]
pub fn admin(_admin: InclusiveAdmin, conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let local_inst = Instance::get_local()?;
    Ok(render!(instance::admin(
        &(&conn, &rockets).to_context(),
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
pub fn admin_mod(_mod: Moderator, conn: DbConn, rockets: PlumeRocket) -> Ructe {
    render!(instance::admin_mod(&(&conn, &rockets).to_context()))
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
    form: LenientForm<InstanceSettingsForm>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> RespondOrRedirect {
    if let Err(e) = form.validate() {
        let local_inst =
            Instance::get_local().expect("instance::update_settings: local instance error");
        render!(instance::admin(
            &(&conn, &rockets).to_context(),
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
                &conn,
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
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let instances = Instance::page(&conn, page.limits())?;
    Ok(render!(instance::list(
        &(&conn, &rockets).to_context(),
        Instance::get_local()?,
        instances,
        page.0,
        Page::total(Instance::count(&conn)? as i32)
    )))
}

#[post("/admin/instances/<id>/block")]
pub fn toggle_block(
    _mod: Moderator,
    conn: DbConn,
    id: i32,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    let inst = Instance::get(&conn, id)?;
    let message = if inst.blocked {
        i18n!(intl.catalog, "{} has been unblocked."; &inst.name)
    } else {
        i18n!(intl.catalog, "{} has been blocked."; &inst.name)
    };

    inst.toggle_block(&conn)?;
    Ok(Flash::success(
        Redirect::to(uri!(admin_instances: page = _)),
        message,
    ))
}

#[get("/admin/users?<page>", rank = 2)]
pub fn admin_users(
    _mod: Moderator,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    Ok(render!(instance::users(
        &(&conn, &rockets).to_context(),
        User::get_local_page(&conn, page.limits())?,
        None,
        page.0,
        Page::total(User::count_local(&conn)? as i32)
    )))
}
#[get("/admin/users?<user>&<page>", rank = 1)]
pub fn admin_search_users(
    _mod: Moderator,
    user: String,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let users = if user.is_empty() {
        User::get_local_page(&conn, page.limits())?
    } else {
        User::search_local_by_name(&conn, &user, page.limits())?
    };

    Ok(render!(instance::users(
        &(&conn, &rockets).to_context(),
        users,
        Some(user.as_str()),
        page.0,
        Page::total(User::count_local(&conn)? as i32)
    )))
}
pub struct BlocklistEmailDeletion {
    ids: Vec<i32>,
}
impl<'f> FromForm<'f> for BlocklistEmailDeletion {
    type Error = ();
    fn from_form(items: &mut FormItems<'f>, _strict: bool) -> Result<BlocklistEmailDeletion, ()> {
        let mut c: BlocklistEmailDeletion = BlocklistEmailDeletion { ids: Vec::new() };
        for item in items {
            let key = item.key.parse::<i32>();
            if let Ok(i) = key {
                c.ids.push(i);
            }
        }
        Ok(c)
    }
}
#[post("/admin/emails/delete", data = "<form>")]
pub fn delete_email_blocklist(
    _mod: Moderator,
    form: Form<BlocklistEmailDeletion>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    BlocklistedEmail::delete_entries(&conn, form.0.ids)?;
    Ok(Flash::success(
        Redirect::to(uri!(admin_email_blocklist: page = None)),
        i18n!(rockets.intl.catalog, "Blocks deleted"),
    ))
}

#[post("/admin/emails/new", data = "<form>")]
pub fn add_email_blocklist(
    _mod: Moderator,
    form: LenientForm<NewBlocklistedEmail>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let result = BlocklistedEmail::insert(&conn, form.0);

    if let Err(Error::Db(_)) = result {
        Ok(Flash::error(
            Redirect::to(uri!(admin_email_blocklist: page = None)),
            i18n!(rockets.intl.catalog, "Email already blocked"),
        ))
    } else {
        Ok(Flash::success(
            Redirect::to(uri!(admin_email_blocklist: page = None)),
            i18n!(rockets.intl.catalog, "Email Blocked"),
        ))
    }
}
#[get("/admin/emails?<page>")]
pub fn admin_email_blocklist(
    _mod: Moderator,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    Ok(render!(instance::emailblocklist(
        &(&conn, &rockets).to_context(),
        BlocklistedEmail::page(&conn, page.limits())?,
        page.0,
        Page::total(BlocklistedEmail::count(&conn)? as i32)
    )))
}

/// A structure to handle forms that are a list of items on which actions are applied.
///
/// This is for instance the case of the user list in the administration.
pub struct MultiAction<T>
where
    T: FromStr,
{
    ids: Vec<i32>,
    action: T,
}

impl<'f, T> FromForm<'f> for MultiAction<T>
where
    T: FromStr,
{
    type Error = ();

    fn from_form(items: &mut FormItems<'_>, _strict: bool) -> Result<Self, Self::Error> {
        let (ids, act) = items.fold((vec![], None), |(mut ids, act), item| {
            let (name, val) = item.key_value_decoded();

            if name == "action" {
                (ids, T::from_str(&val).ok())
            } else if let Ok(id) = name.parse::<i32>() {
                ids.push(id);
                (ids, act)
            } else {
                (ids, act)
            }
        });

        if let Some(act) = act {
            Ok(MultiAction { ids, action: act })
        } else {
            Err(())
        }
    }
}

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
pub fn edit_users(
    moderator: Moderator,
    form: LenientForm<MultiAction<UserActions>>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    // you can't change your own rights
    if form.ids.contains(&moderator.0.id) {
        return Ok(Flash::error(
            Redirect::to(uri!(admin_users: page = _)),
            i18n!(rockets.intl.catalog, "You can't change your own rights."),
        ));
    }

    // moderators can't grant or revoke admin rights
    if !moderator.0.is_admin() {
        match form.action {
            UserActions::Admin | UserActions::RevokeAdmin => {
                return Ok(Flash::error(
                    Redirect::to(uri!(admin_users: page = _)),
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
                User::get(&conn, u)?.set_role(&conn, Role::Admin)?;
            }
        }
        UserActions::Moderator => {
            for u in form.ids.clone() {
                User::get(&conn, u)?.set_role(&conn, Role::Moderator)?;
            }
        }
        UserActions::RevokeAdmin | UserActions::RevokeModerator => {
            for u in form.ids.clone() {
                User::get(&conn, u)?.set_role(&conn, Role::Normal)?;
            }
        }
        UserActions::Ban => {
            for u in form.ids.clone() {
                ban(u, &conn, worker)?;
            }
        }
    }

    Ok(Flash::success(
        Redirect::to(uri!(admin_users: page = _)),
        i18n!(rockets.intl.catalog, "Done."),
    ))
}

fn ban(id: i32, conn: &Connection, worker: &ScheduledThreadPool) -> Result<(), ErrorPage> {
    let u = User::get(conn, id)?;
    u.delete(conn)?;
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
pub fn shared_inbox(
    conn: DbConn,
    data: inbox::SignedJson<serde_json::Value>,
    headers: Headers<'_>,
) -> Result<String, status::BadRequest<&'static str>> {
    inbox::handle_incoming(conn, data, headers)
}

#[get("/remote_interact?<target>")]
pub fn interact(conn: DbConn, user: Option<User>, target: String) -> Option<Redirect> {
    if User::find_by_fqn(&conn, &target).is_ok() {
        return Some(Redirect::to(uri!(super::user::details: name = target)));
    }

    if let Ok(post) = Post::from_id(&conn, &target, None, CONFIG.proxy()) {
        return Some(Redirect::to(uri!(
            super::posts::details: blog = post.get_blog(&conn).expect("Can't retrieve blog").fqn,
            slug = &post.slug,
            responding_to = _
        )));
    }

    if let Ok(comment) = Comment::from_id(&conn, &target, None, CONFIG.proxy()) {
        if comment.can_see(&conn, user.as_ref()) {
            let post = comment.get_post(&conn).expect("Can't retrieve post");
            return Some(Redirect::to(uri!(
                super::posts::details: blog =
                    post.get_blog(&conn).expect("Can't retrieve blog").fqn,
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
                "total": User::count_local(&conn)?
            },
            "localPosts": Post::count_local(&conn)?,
            "localComments": Comment::count_local(&conn)?
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
pub fn about(conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    Ok(render!(instance::about(
        &(&conn, &rockets).to_context(),
        Instance::get_local()?,
        Instance::get_local()?.main_admin(&conn)?,
        User::count_local(&conn)?,
        Post::count_local(&conn)?,
        Instance::count(&conn)? - 1
    )))
}

#[get("/privacy")]
pub fn privacy(conn: DbConn, rockets: PlumeRocket) -> Ructe {
    render!(instance::privacy(&(&conn, &rockets).to_context()))
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
            .map(|i| i.with_prefix(&uri!(static_files: file = "").to_string()))
            .collect::<Vec<_>>()
    })))
}
