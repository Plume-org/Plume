use activitystreams::{
    collection::{OrderedCollection, OrderedCollectionPage},
    iri_string::types::IriString,
    prelude::*,
};
use rocket::{
    form::Form,
    http::{ContentType, CookieJar},
    response::{status, Flash, Redirect},
};
use rocket_i18n::I18n;
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::inbox as crate_inbox;
use crate::routes::{
    email_signups::EmailSignupForm, errors::ErrorPage, Page, RemoteForm, RespondOrRedirect,
};
use crate::template_utils::{default_avatar, IntoContext, PostCard, Ructe};
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
        Some(user) => Redirect::to(uri!(details(name = user.username))).into(),
        None => requires_login("", uri!(me)).into(),
    }
}

#[get("/@/<name>", rank = 2)]
pub async fn details(name: &str, rockets: PlumeRocket, mut conn: DbConn) -> Result<Ructe, ErrorPage> {
    let user = User::find_by_fqn(&mut conn, name).await?;
    let avatar_url = default_avatar(&user.avatar_url(&mut conn)).to_string();
    let recents = Post::get_recents_for_author(&mut conn, &user, 6)?;
    let pc_recents = PostCard::from_posts(&mut conn, recents, &rockets.user);

    if !user.get_instance(&mut conn)?.local {
        tracing::trace!("remote user found");
        user.remote_user_found(); // Doesn't block
    }

    let is_following = rockets.user
        .clone()
        .and_then(|x| x.is_following(&mut conn, user.id).ok())
        .unwrap_or(false);
    let is_remote = user.instance_id != Instance::get_local()?.id;
    let public_domain = user.get_instance(&mut conn)?.public_domain;
    let reshared = Reshare::get_recents_for_author(&mut conn, &user, 6)?
        .into_iter()
        .filter_map(|r| r.get_post(&mut conn).ok())
        .collect();
    let pc_reshared = PostCard::from_posts(&mut conn, reshared, &rockets.user);

    Ok(render!(users::details_html(
        &(&mut conn, &rockets).to_context(),
        user,
        avatar_url,
        is_following,
        is_remote,
        public_domain,
        pc_recents,
        pc_reshared
    )))
}

#[get("/dashboard")]
pub fn dashboard(user: User, mut conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let blogs = Blog::find_for_author(&mut conn, &user)?.into_iter().map(|b| {
        let banner = b.banner_url(&mut conn).unwrap_or_default();
        (b, banner)
    }).collect();

    let drafts = Post::drafts_by_author(&mut conn, &user)?;
    let pc = PostCard::from_posts(&mut conn, drafts, &rockets.user);

    Ok(render!(users::dashboard_html(
        &(&mut conn, &rockets).to_context(),
        blogs,
        pc
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
pub async fn follow(
    name: &str,
    user: User,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let target = User::find_by_fqn(&mut conn, name).await?;
    let message = if let Ok(follow) = follows::Follow::find(&mut conn, user.id, target.id) {
        let delete_act = follow.build_undo(&mut conn)?;
        local_inbox(
            &mut conn,
            serde_json::to_value(&delete_act).map_err(Error::from)?,
        ).await?;

        let msg = i18n!(rockets.intl.catalog, "You are no longer following {}."; target.name());
        rockets
            .worker
            .execute(move || broadcast(&user, delete_act, vec![target], CONFIG.proxy().cloned()));
        msg
    } else {
        let f = follows::Follow::insert(
            &mut conn,
            follows::NewFollow {
                follower_id: user.id,
                following_id: target.id,
                ap_url: String::new(),
            },
        )?;
        f.notify(&mut conn)?;

        let act = f.to_activity(&mut conn)?;
        let msg = i18n!(rockets.intl.catalog, "You are now following {}."; target.name());
        rockets
            .worker
            .execute(move || broadcast(&user, act, vec![target], CONFIG.proxy().cloned()));
        msg
    };
    Ok(Flash::success(
        Redirect::to(uri!(details(name = name))),
        message,
    ))
}

#[post("/@/<name>/follow", data = "<remote_form>", rank = 2)]
pub async fn follow_not_connected(
    mut conn: DbConn,
    rockets: PlumeRocket,
    name: &str,
    remote_form: Option<Form<RemoteForm>>,
    i18n: I18n,
) -> Result<RespondOrRedirect, ErrorPage> {
    let target = User::find_by_fqn(&mut conn, name).await?;
    let avatar_url = target.avatar_url(&mut conn);
    if let Some(remote_form) = remote_form {
        if let Some(uri) = User::fetch_remote_interact_uri(&remote_form).await
            .ok()
            .and_then(|uri| {
                let encoded = rocket::http::RawStr::new(&target.acct_authority(&mut conn).ok()?).percent_encode().to_string();
                Some(uri.replace("{uri}", &encoded))
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
                render!(users::follow_remote_html(
                    &(&mut conn, &rockets).to_context(),
                    target,
                    avatar_url,
                    super::session::LoginForm::default(),
                    ValidationErrors::default(),
                    remote_form.clone(),
                    err
                )),
                "callback",
                uri!(follow(name = name)).to_string(),
            )
            .into())
        }
    } else {
        Ok(Flash::new(
            render!(users::follow_remote_html(
                &(&mut conn, &rockets).to_context(),
                target,
                avatar_url,
                super::session::LoginForm::default(),
                ValidationErrors::default(),
                #[allow(clippy::map_clone)]
                remote_form.map(|x| x.clone()).unwrap_or_default(),
                ValidationErrors::default()
            )),
            "callback",
            uri!(follow(name = name)).to_string(),
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
        uri!(follow(name = name)),
    )
}

#[get("/@/<name>/followers?<page>", rank = 2)]
pub async fn followers(
    name: &str,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let user = User::find_by_fqn(&mut conn, name).await?;
    let avatar_url = user.avatar_url(&mut conn);
    let followers_count = user.count_followers(&mut conn)?;
    let is_following = rockets.user
        .clone()
        .and_then(|x| x.is_following(&mut conn, user.id).ok())
        .unwrap_or(false);
    let is_remote = user.instance_id != Instance::get_local()?.id;
    let public_domain = user.get_instance(&mut conn)?.public_domain;
    let followers_page = user.get_followers_page(&mut conn, page.limits())?;
    let page_total = Page::total(followers_count as i32);

    Ok(render!(users::followers_html(
        &(&mut conn, &rockets).to_context(),
        user,
        avatar_url,
        is_following,
        is_remote,
        public_domain,
        followers_page,
        page.0,
        page_total
    )))
}

#[get("/@/<name>/followed?<page>", rank = 2)]
pub async fn followed(
    name: &str,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let user = User::find_by_fqn(&mut conn, name).await?;
    let avatar_url = user.avatar_url(&mut conn);
    let followed_count = user.count_followed(&mut conn)?;

    let is_following = rockets.user
        .clone()
        .and_then(|x| x.is_following(&mut conn, user.id).ok())
        .unwrap_or(false);
    let is_remote = user.instance_id != Instance::get_local()?.id;

    let public_domain = user.get_instance(&mut conn)?.public_domain;
    let followed_page = user.get_followed_page(&mut conn, page.limits())?;
    let page_total = Page::total(followed_count as i32);

    Ok(render!(users::followed_html(
        &(&mut conn, &rockets).to_context(),
        user,
        avatar_url,
        is_following,
        is_remote,
        public_domain,
        followed_page,
        page.0,
        page_total
    )))
}

#[get("/@/<name>", rank = 1)]
pub async fn activity_details(
    name: String,
    mut conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomPerson>> {
    let user = User::find_by_fqn(&mut conn, &name).await.ok()?;
    Some(ActivityStream::new(user.to_activity(&mut conn).ok()?))
}

#[get("/users/new")]
pub fn new(mut conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    use SignupStrategy::*;

    let rendered = match CONFIG.signup {
        Password => render!(users::new_html(
            &(&mut conn, &rockets).to_context(),
            Instance::get_local()?.open_registrations,
            &NewUserForm::default(),
            ValidationErrors::default()
        )),
        Email => render!(email_signups::new_html(
            &(&mut conn, &rockets).to_context(),
            Instance::get_local()?.open_registrations,
            &EmailSignupForm::default(),
            ValidationErrors::default()
        )),
    };
    Ok(rendered)
}

#[get("/@/<name>/edit")]
pub fn edit(
    name: &str,
    user: User,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    if user.username == name && !name.contains('@') {
        Ok(render!(users::edit_html(
            &(&mut conn, &rockets).to_context(),
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
pub fn edit_auth(name: &str, i18n: I18n) -> Flash<Redirect> {
    requires_login(
        &i18n!(
            i18n.catalog,
            "To edit your profile, you need to be logged in"
        ),
        uri!(edit(name = name)),
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
    name: &str,
    mut conn: DbConn,
    mut user: User,
    form: Form<UpdateUserForm>,
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
            Some(Media::get_media_processor(&mut conn, vec![&user])),
        )
        .0,
    );
    user.preferred_theme = form
        .theme
        .clone()
        .and_then(|t| if t.is_empty() { None } else { Some(t) });
    user.hide_custom_css = form.hide_custom_css;
    user.save(&mut conn)?;

    Ok(Flash::success(
        Redirect::to(uri!(me)),
        i18n!(intl.catalog, "Your profile has been updated."),
    ))
}

#[post("/@/<name>/delete")]
pub async fn delete(
    name: &str,
    user: User,
    cookies: &CookieJar<'_>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let account = User::find_by_fqn(&mut conn, name).await?;
    if user.id == account.id {
        account.delete(&mut conn).await?;

        let target = User::one_by_instance(&mut conn)?;
        let delete_act = account.delete_activity(&mut conn)?;
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
            Redirect::to(uri!(edit(name = name))),
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
    form: Form<NewUserForm>,
    mut conn: DbConn,
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
                &mut conn,
                form.username.to_string(),
                form.username.to_string(),
                Role::Normal,
                "",
                form.email.to_string(),
                Some(User::hash_pass(&form.password).map_err(to_validation)?),
            ).map_err(to_validation)?;
            Ok(Flash::success(
                Redirect::to(uri!(super::session::new(m = _))),
                i18n!(
                    rockets.intl.catalog,
                    "Your account has been created. Now you just need to log in, before you can use it."
                ),
            ))
        })
        .map_err(|err| {
            render!(users::new_html(
                &(&mut conn, &rockets).to_context(),
                Instance::get_local()
                    .map(|i| i.open_registrations)
                    .unwrap_or(true),
                &form,
                err
            ))
        })
}

#[get("/@/<name>/outbox")]
pub async fn outbox(name: &str, mut conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_by_fqn(&mut conn, name).await.ok()?;
    user.outbox(&mut conn).ok()
}
#[get("/@/<name>/outbox?<page>")]
pub async fn outbox_page(
    name: &str,
    page: Page,
    mut conn: DbConn,
) -> Option<ActivityStream<OrderedCollectionPage>> {
    let user = User::find_by_fqn(&mut conn, name).await.ok()?;
    user.outbox_page(&mut conn, page.limits()).ok()
}
#[post("/@/<name>/inbox", data = "<data>")]
pub async fn inbox(
    name: &str,
    data: crate::inbox::SignedJson<serde_json::Value>,
    headers: Headers<'_>,
    mut conn: DbConn,
) -> Result<String, status::BadRequest<&'static str>> {
    User::find_by_fqn(&mut conn, name).await.map_err(|_| status::BadRequest("User not found"))?;
    crate_inbox::handle_incoming(conn, data, headers).await
}

#[get("/@/<name>/followers", rank = 1)]
pub async fn ap_followers(
    name: &str,
    mut conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_by_fqn(&mut conn, name).await.ok()?;
    let followers = user
        .get_followers(&mut conn)
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
pub async fn atom_feed(name: &str, mut conn: DbConn) -> Option<(ContentType, String)> {
    let conn = &mut conn;
    let author = User::find_by_fqn(conn, name).await.ok()?;
    let entries = Post::get_recents_for_author(conn, &author, 15).ok()?;
    let uri = Instance::get_local()
        .ok()?
        .compute_box("@", name, "atom.xml");
    let title = &author.display_name;
    let default_updated = &author.creation_date;
    let feed = super::build_atom_feed(entries, &uri, title, default_updated, conn);
    Some((
        ContentType::new("application", "atom+xml"),
        feed.to_string(),
    ))
}
