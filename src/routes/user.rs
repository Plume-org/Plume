use activitystreams::{
    collection::{OrderedCollection, OrderedCollectionPage},
    iri_string::types::IriString,
    prelude::*,
};
use diesel::SaveChangesDsl;
use rocket::{
    http::{uri::Uri, ContentType, Cookies},
    request::LenientForm,
    response::{status, Content, Flash, Redirect},
};
use rocket_i18n::I18n;
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::inbox;
use crate::routes::{
    email_signups::EmailSignupForm, errors::ErrorPage, Page, RemoteForm, RespondOrRedirect,
};
use crate::template_utils::{IntoContext, Ructe};
use crate::utils::requires_login;
use plume_common::activity_pub::{broadcast, ActivityStream, ApRequest, CustomPerson};
use plume_common::utils::md_to_html;
use plume_models::{
    blogs::Blog,
    db_conn::DbConn,
    follows,
    headers::Headers,
    inbox::inbox as local_inbox,
    instance::Instance,
    medias::Media,
    posts::Post,
    reshares::Reshare,
    safe_string::SafeString,
    signups::{self, Strategy as SignupStrategy},
    users::*,
    Error, PlumeRocket, CONFIG,
};

#[get("/me")]
pub fn me(user: Option<User>) -> RespondOrRedirect {
    match user {
        Some(user) => Redirect::to(uri!(details: name = user.username)).into(),
        None => requires_login("", uri!(me)).into(),
    }
}

#[get("/@/<name>", rank = 2)]
pub fn details(name: String, rockets: PlumeRocket, conn: DbConn) -> Result<Ructe, ErrorPage> {
    let user = User::find_by_fqn(&conn, &name)?;
    let recents = Post::get_recents_for_author(&*conn, &user, 6)?;
    let reshares = Reshare::get_recents_for_author(&*conn, &user, 6)?;

    if !user.get_instance(&*conn)?.local {
        tracing::trace!("remote user found");
        user.remote_user_found(); // Doesn't block
    }

    Ok(render!(users::details(
        &(&conn, &rockets).to_context(),
        user.clone(),
        rockets
            .user
            .clone()
            .and_then(|x| x.is_following(&*conn, user.id).ok())
            .unwrap_or(false),
        user.instance_id != Instance::get_local()?.id,
        user.get_instance(&*conn)?.public_domain,
        recents,
        reshares
            .into_iter()
            .filter_map(|r| r.get_post(&*conn).ok())
            .collect()
    )))
}

#[get("/dashboard")]
pub fn dashboard(user: User, conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let blogs = Blog::find_for_author(&conn, &user)?;
    Ok(render!(users::dashboard(
        &(&conn, &rockets).to_context(),
        blogs,
        Post::drafts_by_author(&conn, &user)?
    )))
}

#[get("/dashboard", rank = 2)]
pub fn dashboard_auth(i18n: I18n) -> Flash<Redirect> {
    requires_login(
        &i18n!(
            i18n.catalog,
            "To access your dashboard, you need to be logged in"
        ),
        uri!(dashboard),
    )
}

#[post("/@/<name>/follow")]
pub fn follow(
    name: String,
    user: User,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let target = User::find_by_fqn(&conn, &name)?;
    let message = if let Ok(follow) = follows::Follow::find(&conn, user.id, target.id) {
        let delete_act = follow.build_undo(&conn)?;
        local_inbox(
            &conn,
            serde_json::to_value(&delete_act).map_err(Error::from)?,
        )?;

        let msg = i18n!(rockets.intl.catalog, "You are no longer following {}."; target.name());
        rockets
            .worker
            .execute(move || broadcast(&user, delete_act, vec![target], CONFIG.proxy().cloned()));
        msg
    } else {
        let f = follows::Follow::insert(
            &conn,
            follows::NewFollow {
                follower_id: user.id,
                following_id: target.id,
                ap_url: String::new(),
            },
        )?;
        f.notify(&conn)?;

        let act = f.to_activity(&conn)?;
        let msg = i18n!(rockets.intl.catalog, "You are now following {}."; target.name());
        rockets
            .worker
            .execute(move || broadcast(&user, act, vec![target], CONFIG.proxy().cloned()));
        msg
    };
    Ok(Flash::success(
        Redirect::to(uri!(details: name = name)),
        message,
    ))
}

#[post("/@/<name>/follow", data = "<remote_form>", rank = 2)]
pub fn follow_not_connected(
    conn: DbConn,
    rockets: PlumeRocket,
    name: String,
    remote_form: Option<LenientForm<RemoteForm>>,
    i18n: I18n,
) -> Result<RespondOrRedirect, ErrorPage> {
    let target = User::find_by_fqn(&conn, &name)?;
    if let Some(remote_form) = remote_form {
        if let Some(uri) = User::fetch_remote_interact_uri(&remote_form)
            .ok()
            .and_then(|uri| {
                Some(uri.replace(
                    "{uri}",
                    &Uri::percent_encode(&target.acct_authority(&conn).ok()?),
                ))
            })
        {
            Ok(Redirect::to(uri).into())
        } else {
            let mut err = ValidationErrors::default();
            err.add("remote",
                ValidationError {
                    code: Cow::from("invalid_remote"),
                    message: Some(Cow::from(i18n!(&i18n.catalog, "Couldn't obtain enough information about your account. Please make sure your username is correct."))),
                    params: HashMap::new(),
                },
            );
            Ok(Flash::new(
                render!(users::follow_remote(
                    &(&conn, &rockets).to_context(),
                    target,
                    super::session::LoginForm::default(),
                    ValidationErrors::default(),
                    remote_form.clone(),
                    err
                )),
                "callback",
                uri!(follow: name = name).to_string(),
            )
            .into())
        }
    } else {
        Ok(Flash::new(
            render!(users::follow_remote(
                &(&conn, &rockets).to_context(),
                target,
                super::session::LoginForm::default(),
                ValidationErrors::default(),
                #[allow(clippy::map_clone)]
                remote_form.map(|x| x.clone()).unwrap_or_default(),
                ValidationErrors::default()
            )),
            "callback",
            uri!(follow: name = name).to_string(),
        )
        .into())
    }
}

#[get("/@/<name>/follow?local", rank = 2)]
pub fn follow_auth(name: String, i18n: I18n) -> Flash<Redirect> {
    requires_login(
        &i18n!(
            i18n.catalog,
            "To subscribe to someone, you need to be logged in"
        ),
        uri!(follow: name = name),
    )
}

#[get("/@/<name>/followers?<page>", rank = 2)]
pub fn followers(
    name: String,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let user = User::find_by_fqn(&conn, &name)?;
    let followers_count = user.count_followers(&conn)?;

    Ok(render!(users::followers(
        &(&conn, &rockets).to_context(),
        user.clone(),
        rockets
            .user
            .clone()
            .and_then(|x| x.is_following(&conn, user.id).ok())
            .unwrap_or(false),
        user.instance_id != Instance::get_local()?.id,
        user.get_instance(&conn)?.public_domain,
        user.get_followers_page(&conn, page.limits())?,
        page.0,
        Page::total(followers_count as i32)
    )))
}

#[get("/@/<name>/followed?<page>", rank = 2)]
pub fn followed(
    name: String,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let user = User::find_by_fqn(&conn, &name)?;
    let followed_count = user.count_followed(&conn)?;

    Ok(render!(users::followed(
        &(&conn, &rockets).to_context(),
        user.clone(),
        rockets
            .user
            .clone()
            .and_then(|x| x.is_following(&conn, user.id).ok())
            .unwrap_or(false),
        user.instance_id != Instance::get_local()?.id,
        user.get_instance(&conn)?.public_domain,
        user.get_followed_page(&conn, page.limits())?,
        page.0,
        Page::total(followed_count as i32)
    )))
}

#[get("/@/<name>", rank = 1)]
pub fn activity_details(
    name: String,
    conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomPerson>> {
    let user = User::find_by_fqn(&conn, &name).ok()?;
    Some(ActivityStream::new(user.to_activity(&conn).ok()?))
}

#[get("/users/new")]
pub fn new(conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    use SignupStrategy::*;

    let rendered = match CONFIG.signup {
        Password => render!(users::new(
            &(&conn, &rockets).to_context(),
            Instance::get_local()?.open_registrations,
            &NewUserForm::default(),
            ValidationErrors::default()
        )),
        Email => render!(email_signups::new(
            &(&conn, &rockets).to_context(),
            Instance::get_local()?.open_registrations,
            &EmailSignupForm::default(),
            ValidationErrors::default()
        )),
    };
    Ok(rendered)
}

#[get("/@/<name>/edit")]
pub fn edit(
    name: String,
    user: User,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    if user.username == name && !name.contains('@') {
        Ok(render!(users::edit(
            &(&conn, &rockets).to_context(),
            UpdateUserForm {
                display_name: user.display_name.clone(),
                email: user.email.clone().unwrap_or_default(),
                summary: user.summary.clone(),
                theme: user.preferred_theme,
                hide_custom_css: user.hide_custom_css,
            },
            ValidationErrors::default()
        )))
    } else {
        Err(Error::Unauthorized.into())
    }
}

#[get("/@/<name>/edit", rank = 2)]
pub fn edit_auth(name: String, i18n: I18n) -> Flash<Redirect> {
    requires_login(
        &i18n!(
            i18n.catalog,
            "To edit your profile, you need to be logged in"
        ),
        uri!(edit: name = name),
    )
}

#[derive(FromForm)]
pub struct UpdateUserForm {
    pub display_name: String,
    pub email: String,
    pub summary: String,
    pub theme: Option<String>,
    pub hide_custom_css: bool,
}

#[allow(unused_variables)]
#[put("/@/<name>/edit", data = "<form>")]
pub fn update(
    name: String,
    conn: DbConn,
    mut user: User,
    form: LenientForm<UpdateUserForm>,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    user.display_name = form.display_name.clone();
    user.email = Some(form.email.clone());
    user.summary = form.summary.clone();
    user.summary_html = SafeString::new(
        &md_to_html(
            &form.summary,
            None,
            false,
            Some(Media::get_media_processor(&conn, vec![&user])),
        )
        .0,
    );
    user.preferred_theme = form
        .theme
        .clone()
        .and_then(|t| if t.is_empty() { None } else { Some(t) });
    user.hide_custom_css = form.hide_custom_css;
    let _: User = user.save_changes(&*conn).map_err(Error::from)?;

    Ok(Flash::success(
        Redirect::to(uri!(me)),
        i18n!(intl.catalog, "Your profile has been updated."),
    ))
}

#[post("/@/<name>/delete")]
pub fn delete(
    name: String,
    user: User,
    mut cookies: Cookies<'_>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let account = User::find_by_fqn(&conn, &name)?;
    if user.id == account.id {
        account.delete(&conn)?;

        let target = User::one_by_instance(&conn)?;
        let delete_act = account.delete_activity(&conn)?;
        rockets
            .worker
            .execute(move || broadcast(&account, delete_act, target, CONFIG.proxy().cloned()));

        if let Some(cookie) = cookies.get_private(AUTH_COOKIE) {
            cookies.remove_private(cookie);
        }

        Ok(Flash::success(
            Redirect::to(uri!(super::instance::index)),
            i18n!(rockets.intl.catalog, "Your account has been deleted."),
        ))
    } else {
        Ok(Flash::error(
            Redirect::to(uri!(edit: name = name)),
            i18n!(
                rockets.intl.catalog,
                "You can't delete someone else's account."
            ),
        ))
    }
}

#[derive(Default, FromForm, Validate)]
#[validate(schema(
    function = "passwords_match",
    skip_on_field_errors = false,
    message = "Passwords are not matching"
))]
pub struct NewUserForm {
    #[validate(
        length(min = 1, message = "Username can't be empty"),
        custom(
            function = "validate_username",
            message = "User name is not allowed to contain any of < > & @ ' or \""
        )
    )]
    pub username: String,
    #[validate(email(message = "Invalid email"))]
    pub email: String,
    #[validate(length(min = 8, message = "Password should be at least 8 characters long"))]
    pub password: String,
    #[validate(length(min = 8, message = "Password should be at least 8 characters long"))]
    pub password_confirmation: String,
}

pub fn passwords_match(form: &NewUserForm) -> Result<(), ValidationError> {
    if form.password != form.password_confirmation {
        Err(ValidationError::new("password_match"))
    } else {
        Ok(())
    }
}

pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    if username.contains(&['<', '>', '&', '@', '\'', '"', ' ', '\n', '\t'][..]) {
        Err(ValidationError::new("username_illegal_char"))
    } else {
        Ok(())
    }
}

fn to_validation(x: Error) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    if let Error::Blocklisted(show, msg) = x {
        if show {
            errors.add(
                "email",
                ValidationError {
                    code: Cow::from("blocklisted"),
                    message: Some(Cow::from(msg)),
                    params: HashMap::new(),
                },
            );
        }
    }
    errors.add(
        "",
        ValidationError {
            code: Cow::from("server_error"),
            message: Some(Cow::from("An unknown error occured")),
            params: HashMap::new(),
        },
    );
    errors
}

#[post("/users/new", data = "<form>")]
pub fn create(
    form: LenientForm<NewUserForm>,
    conn: DbConn,
    rockets: PlumeRocket,
    _enabled: signups::Password,
) -> Result<Flash<Redirect>, Ructe> {
    if !Instance::get_local()
        .map(|i| i.open_registrations)
        .unwrap_or(true)
    {
        return Ok(Flash::error(
            Redirect::to(uri!(new)),
            i18n!(
                rockets.intl.catalog,
                "Registrations are closed on this instance."
            ),
        )); // Actually, it is an error
    }

    let mut form = form.into_inner();
    form.username = form.username.trim().to_owned();
    form.email = form.email.trim().to_owned();
    form.validate()
        .and_then(|_| {
            NewUser::new_local(
                &conn,
                form.username.to_string(),
                form.username.to_string(),
                Role::Normal,
                "",
                form.email.to_string(),
                Some(User::hash_pass(&form.password).map_err(to_validation)?),
            ).map_err(to_validation)?;
            Ok(Flash::success(
                Redirect::to(uri!(super::session::new: m = _)),
                i18n!(
                    rockets.intl.catalog,
                    "Your account has been created. Now you just need to log in, before you can use it."
                ),
            ))
        })
        .map_err(|err| {
            render!(users::new(
                &(&conn, &rockets).to_context(),
                Instance::get_local()
                    .map(|i| i.open_registrations)
                    .unwrap_or(true),
                &form,
                err
            ))
        })
}

#[get("/@/<name>/outbox")]
pub fn outbox(name: String, conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_by_fqn(&conn, &name).ok()?;
    user.outbox(&conn).ok()
}
#[get("/@/<name>/outbox?<page>")]
pub fn outbox_page(
    name: String,
    page: Page,
    conn: DbConn,
) -> Option<ActivityStream<OrderedCollectionPage>> {
    let user = User::find_by_fqn(&conn, &name).ok()?;
    user.outbox_page(&conn, page.limits()).ok()
}
#[post("/@/<name>/inbox", data = "<data>")]
pub fn inbox(
    name: String,
    data: inbox::SignedJson<serde_json::Value>,
    headers: Headers<'_>,
    conn: DbConn,
) -> Result<String, status::BadRequest<&'static str>> {
    User::find_by_fqn(&conn, &name).map_err(|_| status::BadRequest(Some("User not found")))?;
    inbox::handle_incoming(conn, data, headers)
}

#[get("/@/<name>/followers", rank = 1)]
pub fn ap_followers(
    name: String,
    conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_by_fqn(&conn, &name).ok()?;
    let followers = user
        .get_followers(&conn)
        .ok()?
        .into_iter()
        .filter_map(|f| f.ap_url.parse::<IriString>().ok())
        .collect::<Vec<IriString>>();

    let mut coll = OrderedCollection::new();
    coll.set_id(user.followers_endpoint.parse::<IriString>().ok()?);
    coll.set_total_items(followers.len() as u64);
    coll.set_many_items(followers);
    Some(ActivityStream::new(coll))
}

#[get("/@/<name>/atom.xml")]
pub fn atom_feed(name: String, conn: DbConn) -> Option<Content<String>> {
    let conn = &conn;
    let author = User::find_by_fqn(conn, &name).ok()?;
    let entries = Post::get_recents_for_author(conn, &author, 15).ok()?;
    let uri = Instance::get_local()
        .ok()?
        .compute_box("@", &name, "atom.xml");
    let title = &author.display_name;
    let default_updated = &author.creation_date;
    let feed = super::build_atom_feed(entries, &uri, title, default_updated, conn);
    Some(Content(
        ContentType::new("application", "atom+xml"),
        feed.to_string(),
    ))
}
