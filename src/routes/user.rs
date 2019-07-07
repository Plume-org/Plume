use activitypub::{activity::Create, collection::OrderedCollection};
use atom_syndication::{Entry, FeedBuilder};
use rocket::{
    http::{ContentType, Cookies},
    request::LenientForm,
    response::{status, Content, Flash, Redirect},
};
use rocket_i18n::I18n;
use serde_json;
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use inbox;
use plume_common::activity_pub::{broadcast, inbox::FromId, ActivityStream, ApRequest, Id};
use plume_common::utils;
use plume_models::{
    blogs::Blog,
    db_conn::DbConn,
    follows,
    headers::Headers,
    inbox::inbox as local_inbox,
    instance::Instance,
    posts::{LicensedArticle, Post},
    reshares::Reshare,
    users::*,
    Error, PlumeRocket,
};
use routes::{errors::ErrorPage, Page, RemoteForm, RespondOrRedirect};
use template_utils::{IntoContext, Ructe};

#[get("/me")]
pub fn me(user: Option<User>) -> RespondOrRedirect {
    match user {
        Some(user) => Redirect::to(uri!(details: name = user.username)).into(),
        None => utils::requires_login("", uri!(me)).into(),
    }
}

#[get("/@/<name>", rank = 2)]
pub fn details(
    name: String,
    rockets: PlumeRocket,
    fetch_rockets: PlumeRocket,
    fetch_followers_rockets: PlumeRocket,
    update_conn: DbConn,
) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let user = User::find_by_fqn(&rockets, &name)?;
    let recents = Post::get_recents_for_author(&*conn, &user, 6)?;
    let reshares = Reshare::get_recents_for_author(&*conn, &user, 6)?;
    let worker = &rockets.worker;

    if !user.get_instance(&*conn)?.local {
        // Fetch new articles
        let user_clone = user.clone();
        worker.execute(move || {
            for create_act in user_clone
                .fetch_outbox::<Create>()
                .expect("Remote user: outbox couldn't be fetched")
            {
                match create_act.create_props.object_object::<LicensedArticle>() {
                    Ok(article) => {
                        Post::from_activity(&fetch_rockets, article)
                            .expect("Article from remote user couldn't be saved");
                        println!("Fetched article from remote user");
                    }
                    Err(e) => println!("Error while fetching articles in background: {:?}", e),
                }
            }
        });

        // Fetch followers
        let user_clone = user.clone();
        worker.execute(move || {
            for user_id in user_clone
                .fetch_followers_ids()
                .expect("Remote user: fetching followers error")
            {
                let follower = User::from_id(&fetch_followers_rockets, &user_id, None)
                    .expect("user::details: Couldn't fetch follower");
                follows::Follow::insert(
                    &*fetch_followers_rockets.conn,
                    follows::NewFollow {
                        follower_id: follower.id,
                        following_id: user_clone.id,
                        ap_url: String::new(),
                    },
                )
                .expect("Couldn't save follower for remote user");
            }
        });

        // Update profile information if needed
        let user_clone = user.clone();
        if user.needs_update() {
            worker.execute(move || {
                user_clone
                    .refetch(&*update_conn)
                    .expect("Couldn't update user info");
            });
        }
    }

    Ok(render!(users::details(
        &rockets.to_context(),
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
pub fn dashboard(user: User, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let blogs = Blog::find_for_author(&*rockets.conn, &user)?;
    Ok(render!(users::dashboard(
        &rockets.to_context(),
        blogs,
        Post::drafts_by_author(&*rockets.conn, &user)?
    )))
}

#[get("/dashboard", rank = 2)]
pub fn dashboard_auth(i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
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
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let conn = &*rockets.conn;
    let target = User::find_by_fqn(&rockets, &name)?;
    let message = if let Ok(follow) = follows::Follow::find(&*conn, user.id, target.id) {
        let delete_act = follow.build_undo(&*conn)?;
        local_inbox(
            &rockets,
            serde_json::to_value(&delete_act).map_err(Error::from)?,
        )?;

        let msg = i18n!(rockets.intl.catalog, "You are no longer following {}."; target.name());
        rockets
            .worker
            .execute(move || broadcast(&user, delete_act, vec![target]));
        msg
    } else {
        let f = follows::Follow::insert(
            &*conn,
            follows::NewFollow {
                follower_id: user.id,
                following_id: target.id,
                ap_url: String::new(),
            },
        )?;
        f.notify(&*conn)?;

        let act = f.to_activity(&*conn)?;
        let msg = i18n!(rockets.intl.catalog, "You are now following {}."; target.name());
        rockets
            .worker
            .execute(move || broadcast(&user, act, vec![target]));
        msg
    };
    Ok(Flash::success(
        Redirect::to(uri!(details: name = name)),
        message,
    ))
}

#[post("/@/<name>/follow", data = "<remote_form>", rank = 2)]
pub fn follow_not_connected(
    rockets: PlumeRocket,
    name: String,
    remote_form: Option<LenientForm<RemoteForm>>,
    i18n: I18n,
) -> Result<RespondOrRedirect, ErrorPage> {
    let target = User::find_by_fqn(&rockets, &name)?;
    if let Some(remote_form) = remote_form {
        if let Some(uri) = User::fetch_remote_interact_uri(&remote_form)
            .ok()
            .and_then(|uri| {
                rt_format!(
                    uri,
                    uri = format!(
                        "{}@{}",
                        target.fqn,
                        target.get_instance(&rockets.conn).ok()?.public_domain
                    )
                )
                .ok()
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
                    &rockets.to_context(),
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
                &rockets.to_context(),
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
    utils::requires_login(
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
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let page = page.unwrap_or_default();
    let user = User::find_by_fqn(&rockets, &name)?;
    let followers_count = user.count_followers(&*conn)?;

    Ok(render!(users::followers(
        &rockets.to_context(),
        user.clone(),
        rockets
            .user
            .clone()
            .and_then(|x| x.is_following(&*conn, user.id).ok())
            .unwrap_or(false),
        user.instance_id != Instance::get_local()?.id,
        user.get_instance(&*conn)?.public_domain,
        user.get_followers_page(&*conn, page.limits())?,
        page.0,
        Page::total(followers_count as i32)
    )))
}

#[get("/@/<name>/followed?<page>", rank = 2)]
pub fn followed(
    name: String,
    page: Option<Page>,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let page = page.unwrap_or_default();
    let user = User::find_by_fqn(&rockets, &name)?;
    let followed_count = user.count_followed(conn)?;

    Ok(render!(users::followed(
        &rockets.to_context(),
        user.clone(),
        rockets
            .user
            .clone()
            .and_then(|x| x.is_following(conn, user.id).ok())
            .unwrap_or(false),
        user.instance_id != Instance::get_local()?.id,
        user.get_instance(conn)?.public_domain,
        user.get_followed_page(conn, page.limits())?,
        page.0,
        Page::total(followed_count as i32)
    )))
}

#[get("/@/<name>", rank = 1)]
pub fn activity_details(
    name: String,
    rockets: PlumeRocket,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomPerson>> {
    let user = User::find_by_fqn(&rockets, &name).ok()?;
    Some(ActivityStream::new(user.to_activity(&*rockets.conn).ok()?))
}

#[get("/users/new")]
pub fn new(rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    Ok(render!(users::new(
        &rockets.to_context(),
        Instance::get_local()?.open_registrations,
        &NewUserForm::default(),
        ValidationErrors::default()
    )))
}

#[get("/@/<name>/edit")]
pub fn edit(name: String, user: User, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    if user.username == name && !name.contains('@') {
        Ok(render!(users::edit(
            &rockets.to_context(),
            UpdateUserForm {
                display_name: user.display_name.clone(),
                email: user.email.clone().unwrap_or_default(),
                summary: user.summary,
            },
            ValidationErrors::default()
        )))
    } else {
        Err(Error::Unauthorized)?
    }
}

#[get("/@/<name>/edit", rank = 2)]
pub fn edit_auth(name: String, i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
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
}

#[put("/@/<_name>/edit", data = "<form>")]
pub fn update(
    _name: String,
    conn: DbConn,
    user: User,
    form: LenientForm<UpdateUserForm>,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    user.update(
        &*conn,
        if !form.display_name.is_empty() {
            form.display_name.clone()
        } else {
            user.display_name.clone()
        },
        if !form.email.is_empty() {
            form.email.clone()
        } else {
            user.email.clone().unwrap_or_default()
        },
        if !form.summary.is_empty() {
            form.summary.clone()
        } else {
            user.summary.to_string()
        },
    )?;
    Ok(Flash::success(
        Redirect::to(uri!(me)),
        i18n!(intl.catalog, "Your profile has been updated."),
    ))
}

#[post("/@/<name>/delete")]
pub fn delete(
    name: String,
    user: User,
    mut cookies: Cookies,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    let account = User::find_by_fqn(&rockets, &name)?;
    if user.id == account.id {
        account.delete(&*rockets.conn, &rockets.searcher)?;

        let target = User::one_by_instance(&*rockets.conn)?;
        let delete_act = account.delete_activity(&*rockets.conn)?;
        rockets
            .worker
            .execute(move || broadcast(&account, delete_act, target));

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
    skip_on_field_errors = "false",
    message = "Passwords are not matching"
))]
pub struct NewUserForm {
    #[validate(
        length(min = "1", message = "Username can't be empty"),
        custom(
            function = "validate_username",
            message = "User name is not allowed to contain any of < > & @ ' or \""
        )
    )]
    pub username: String,
    #[validate(email(message = "Invalid email"))]
    pub email: String,
    #[validate(length(min = "8", message = "Password should be at least 8 characters long"))]
    pub password: String,
    #[validate(length(min = "8", message = "Password should be at least 8 characters long"))]
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

fn to_validation(_: Error) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
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
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, Ructe> {
    let conn = &*rockets.conn;
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
                conn,
                form.username.to_string(),
                form.username.to_string(),
                false,
                "",
                form.email.to_string(),
                &form.password,
            )
            .map_err(to_validation)?;
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
                &rockets.to_context(),
                Instance::get_local()
                    .map(|i| i.open_registrations)
                    .unwrap_or(true),
                &form,
                err
            ))
        })
}

#[get("/@/<name>/outbox")]
pub fn outbox(name: String, rockets: PlumeRocket) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_by_fqn(&rockets, &name).ok()?;
    user.outbox(&*rockets.conn).ok()
}

#[post("/@/<name>/inbox", data = "<data>")]
pub fn inbox(
    name: String,
    data: inbox::SignedJson<serde_json::Value>,
    headers: Headers,
    rockets: PlumeRocket,
) -> Result<String, status::BadRequest<&'static str>> {
    User::find_by_fqn(&rockets, &name).map_err(|_| status::BadRequest(Some("User not found")))?;
    inbox::handle_incoming(rockets, data, headers)
}

#[get("/@/<name>/followers", rank = 1)]
pub fn ap_followers(
    name: String,
    rockets: PlumeRocket,
    _ap: ApRequest,
) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_by_fqn(&rockets, &name).ok()?;
    let followers = user
        .get_followers(&*rockets.conn)
        .ok()?
        .into_iter()
        .map(|f| Id::new(f.ap_url))
        .collect::<Vec<Id>>();

    let mut coll = OrderedCollection::default();
    coll.object_props
        .set_id_string(user.followers_endpoint)
        .ok()?;
    coll.collection_props
        .set_total_items_u64(followers.len() as u64)
        .ok()?;
    coll.collection_props.set_items_link_vec(followers).ok()?;
    Some(ActivityStream::new(coll))
}

#[get("/@/<name>/atom.xml")]
pub fn atom_feed(name: String, rockets: PlumeRocket) -> Option<Content<String>> {
    let conn = &*rockets.conn;
    let author = User::find_by_fqn(&rockets, &name).ok()?;
    let feed = FeedBuilder::default()
        .title(author.display_name.clone())
        .id(Instance::get_local()
            .unwrap()
            .compute_box("@", &name, "atom.xml"))
        .entries(
            Post::get_recents_for_author(conn, &author, 15)
                .ok()?
                .into_iter()
                .map(|p| super::post_to_atom(p, conn))
                .collect::<Vec<Entry>>(),
        )
        .build()
        .expect("user::atom_feed: Error building Atom feed");
    Some(Content(
        ContentType::new("application", "atom+xml"),
        feed.to_string(),
    ))
}
