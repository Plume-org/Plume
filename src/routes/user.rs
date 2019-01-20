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

use inbox::{Inbox, SignedJson};
use plume_common::activity_pub::{
    broadcast,
    inbox::{Deletable, FromActivity, Notify},
    sign::{verify_http_headers, Signable},
    ActivityStream, ApRequest, Id, IntoId,
};
use plume_common::utils;
use plume_models::{
    Error,
    blogs::Blog, db_conn::DbConn, follows, headers::Headers, instance::Instance, posts::{LicensedArticle, Post},
    reshares::Reshare, users::*,
};
use routes::{Page, errors::ErrorPage};
use template_utils::Ructe;
use Worker;
use Searcher;

#[get("/me")]
pub fn me(user: Option<User>) -> Result<Redirect, Flash<Redirect>> {
    match user {
        Some(user) => Ok(Redirect::to(uri!(details: name = user.username))),
        None => Err(utils::requires_login("", uri!(me))),
    }
}

#[get("/@/<name>", rank = 2)]
pub fn details(
    name: String,
    conn: DbConn,
    account: Option<User>,
    worker: Worker,
    fetch_articles_conn: DbConn,
    fetch_followers_conn: DbConn,
    update_conn: DbConn,
    intl: I18n,
    searcher: Searcher,
) -> Result<Ructe, ErrorPage> {
    let user = User::find_by_fqn(&*conn, &name)?;
    let recents = Post::get_recents_for_author(&*conn, &user, 6)?;
    let reshares = Reshare::get_recents_for_author(&*conn, &user, 6)?;

    if !user.get_instance(&*conn)?.local {
        // Fetch new articles
        let user_clone = user.clone();
        let searcher = searcher.clone();
        worker.execute(move || {
            for create_act in user_clone.fetch_outbox::<Create>().expect("Remote user: outbox couldn't be fetched") {
                match create_act.create_props.object_object::<LicensedArticle>() {
                    Ok(article) => {
                        Post::from_activity(
                            &(&*fetch_articles_conn, &searcher),
                            article,
                            user_clone.clone().into_id(),
                        ).expect("Article from remote user couldn't be saved");
                        println!("Fetched article from remote user");
                    }
                    Err(e) => {
                        println!("Error while fetching articles in background: {:?}", e)
                    }
                }
            }
        });

        // Fetch followers
        let user_clone = user.clone();
        worker.execute(move || {
            for user_id in user_clone.fetch_followers_ids().expect("Remote user: fetching followers error") {
                let follower =
                    User::find_by_ap_url(&*fetch_followers_conn, &user_id)
                        .unwrap_or_else(|_| {
                            User::fetch_from_url(&*fetch_followers_conn, &user_id)
                                .expect("user::details: Couldn't fetch follower")
                        });
                follows::Follow::insert(
                    &*fetch_followers_conn,
                    follows::NewFollow {
                        follower_id: follower.id,
                        following_id: user_clone.id,
                        ap_url: format!("{}/follow/{}", follower.ap_url, user_clone.ap_url),
                    },
                ).expect("Couldn't save follower for remote user");
            }
        });

        // Update profile information if needed
        let user_clone = user.clone();
        if user.needs_update() {
            worker.execute(move || {
                user_clone.refetch(&*update_conn).expect("Couldn't update user info");
            });
        }
    }

    Ok(render!(users::details(
        &(&*conn, &intl.catalog, account.clone()),
        user.clone(),
        account.and_then(|x| x.is_following(&*conn, user.id).ok()).unwrap_or(false),
        user.instance_id != Instance::get_local(&*conn)?.id,
        user.get_instance(&*conn)?.public_domain,
        recents,
        reshares.into_iter().filter_map(|r| r.get_post(&*conn).ok()).collect()
    )))
}

#[get("/dashboard")]
pub fn dashboard(user: User, conn: DbConn, intl: I18n) -> Result<Ructe, ErrorPage> {
    let blogs = Blog::find_for_author(&*conn, &user)?;
    Ok(render!(users::dashboard(
        &(&*conn, &intl.catalog, Some(user.clone())),
        blogs,
        Post::drafts_by_author(&*conn, &user)?
    )))
}

#[get("/dashboard", rank = 2)]
pub fn dashboard_auth(i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(i18n.catalog, "You need to be logged in order to access your dashboard"),
        uri!(dashboard),
    )
}

#[post("/@/<name>/follow")]
pub fn follow(name: String, conn: DbConn, user: User, worker: Worker) -> Result<Redirect, ErrorPage> {
    let target = User::find_by_fqn(&*conn, &name)?;
    if let Ok(follow) = follows::Follow::find(&*conn, user.id, target.id) {
        let delete_act = follow.delete(&*conn)?;
        worker.execute(move || {
            broadcast(&user, delete_act, vec![target])
        });
    } else {
        let f = follows::Follow::insert(
            &*conn,
            follows::NewFollow {
                follower_id: user.id,
                following_id: target.id,
                ap_url: format!("{}/follow/{}", user.ap_url, target.ap_url),
            },
        )?;
        f.notify(&*conn)?;

        let act = f.to_activity(&*conn)?;
        worker.execute(move || broadcast(&user, act, vec![target]));
    }
    Ok(Redirect::to(uri!(details: name = name)))
}

#[post("/@/<name>/follow", rank = 2)]
pub fn follow_auth(name: String, i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(i18n.catalog, "You need to be logged in order to follow someone"),
        uri!(follow: name = name),
    )
}

#[get("/@/<name>/followers?<page>", rank = 2)]
pub fn followers(name: String, conn: DbConn, account: Option<User>, page: Option<Page>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let user = User::find_by_fqn(&*conn, &name)?;
    let followers_count = user.count_followers(&*conn)?;

    Ok(render!(users::followers(
        &(&*conn, &intl.catalog, account.clone()),
        user.clone(),
        account.and_then(|x| x.is_following(&*conn, user.id).ok()).unwrap_or(false),
        user.instance_id != Instance::get_local(&*conn)?.id,
        user.get_instance(&*conn)?.public_domain,
        user.get_followers_page(&*conn, page.limits())?,
        page.0,
        Page::total(followers_count as i32)
    )))
}

#[get("/@/<name>", rank = 1)]
pub fn activity_details(
    name: String,
    conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomPerson>> {
    let user = User::find_local(&*conn, &name).ok()?;
    Some(ActivityStream::new(user.to_activity(&*conn).ok()?))
}

#[get("/users/new")]
pub fn new(user: Option<User>, conn: DbConn, intl: I18n) -> Result<Ructe, ErrorPage> {
    Ok(render!(users::new(
        &(&*conn, &intl.catalog, user),
        Instance::get_local(&*conn)?.open_registrations,
        &NewUserForm::default(),
        ValidationErrors::default()
    )))
}

#[get("/@/<name>/edit")]
pub fn edit(name: String, user: User, conn: DbConn, intl: I18n) -> Result<Ructe, ErrorPage> {
    if user.username == name && !name.contains('@') {
        Ok(render!(users::edit(
            &(&*conn, &intl.catalog, Some(user.clone())),
            UpdateUserForm {
                display_name: user.display_name.clone(),
                email: user.email.clone().unwrap_or_default(),
                summary: user.summary.to_string(),
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
        &i18n!(i18n.catalog, "You need to be logged in order to edit your profile"),
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
pub fn update(_name: String, conn: DbConn, user: User, form: LenientForm<UpdateUserForm>) -> Result<Redirect, ErrorPage> {
    user.update(
        &*conn,
        if !form.display_name.is_empty() { form.display_name.clone() } else { user.display_name.clone() },
        if !form.email.is_empty() { form.email.clone() } else { user.email.clone().unwrap_or_default() },
        if !form.summary.is_empty() { form.summary.clone() } else { user.summary.to_string() },
    )?;
    Ok(Redirect::to(uri!(me)))
}

#[post("/@/<name>/delete")]
pub fn delete(name: String, conn: DbConn, user: User, mut cookies: Cookies, searcher: Searcher) -> Result<Redirect, ErrorPage> {
    let account = User::find_by_fqn(&*conn, &name)?;
    if user.id == account.id {
        account.delete(&*conn, &searcher)?;

        if let Some(cookie) = cookies.get_private(AUTH_COOKIE) {
            cookies.remove_private(cookie);
        }

        Ok(Redirect::to(uri!(super::instance::index)))
    } else {
        Ok(Redirect::to(uri!(edit: name = name)))
    }
}

#[derive(Default, FromForm, Serialize, Validate)]
#[validate(
    schema(
        function = "passwords_match",
        skip_on_field_errors = "false",
        message = "Passwords are not matching"
    )
)]
pub struct NewUserForm {
    #[validate(length(min = "1", message = "Username can't be empty"),
        custom( function = "validate_username", message = "User name is not allowed to contain any of < > & @ ' or \""))]
    pub username: String,
    #[validate(email(message = "Invalid email"))]
    pub email: String,
    #[validate(
        length(
            min = "8",
            message = "Password should be at least 8 characters long"
        )
    )]
    pub password: String,
    #[validate(
        length(
            min = "8",
            message = "Password should be at least 8 characters long"
        )
    )]
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
    errors.add("", ValidationError {
        code: Cow::from("server_error"),
        message: Some(Cow::from("An unknown error occured")),
        params: HashMap::new()
    });
    errors
}

#[post("/users/new", data = "<form>")]
pub fn create(conn: DbConn, form: LenientForm<NewUserForm>, intl: I18n) -> Result<Redirect, Ructe> {
  if !Instance::get_local(&*conn)
        .map(|i| i.open_registrations)
        .unwrap_or(true)
    {
        return Ok(Redirect::to(uri!(new))); // Actually, it is an error
    }

    let mut form = form.into_inner();
    form.username = form.username.trim().to_owned();
    form.email = form.email.trim().to_owned();
    form.validate()
        .and_then(|_| {
            NewUser::new_local(
                &*conn,
                form.username.to_string(),
                form.username.to_string(),
                false,
                "",
                form.email.to_string(),
                User::hash_pass(&form.password).map_err(to_validation)?,
            ).and_then(|u| u.update_boxes(&*conn)).map_err(to_validation)?;
            Ok(Redirect::to(uri!(super::session::new: m = _)))
        })
       .map_err(|err| {
            render!(users::new(
                &(&*conn, &intl.catalog, None),
                Instance::get_local(&*conn).map(|i| i.open_registrations).unwrap_or(true),
                &form,
                err
            ))
        })
}

#[get("/@/<name>/outbox")]
pub fn outbox(name: String, conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_local(&*conn, &name).ok()?;
    user.outbox(&*conn).ok()
}

#[post("/@/<name>/inbox", data = "<data>")]
pub fn inbox(
    name: String,
    conn: DbConn,
    data: SignedJson<serde_json::Value>,
    headers: Headers,
    searcher: Searcher,
) -> Result<String, Option<status::BadRequest<&'static str>>> {
    let user = User::find_local(&*conn, &name).map_err(|_| None)?;
    let act = data.1.into_inner();
    let sig = data.0;

    let activity = act.clone();
    let actor_id = activity["actor"]
        .as_str()
        .or_else(|| activity["actor"]["id"].as_str())
        .ok_or(Some(status::BadRequest(Some(
            "Missing actor id for activity",
        ))))?;

    let actor = User::from_url(&conn, actor_id).expect("user::inbox: user error");
    if !verify_http_headers(&actor, &headers.0, &sig).is_secure()
        && !act.clone().verify(&actor)
    {
        // maybe we just know an old key?
        actor.refetch(&conn).and_then(|_| User::get(&conn, actor.id))
            .and_then(|actor| if verify_http_headers(&actor, &headers.0, &sig).is_secure()
                      || act.clone().verify(&actor)
                    {
                        Ok(())
                    } else {
                        Err(Error::Signature)
                    })
            .map_err(|_| {
                println!("Rejected invalid activity supposedly from {}, with headers {:?}", actor.username, headers.0);
                status::BadRequest(Some("Invalid signature"))})?;
    }

    if Instance::is_blocked(&*conn, actor_id).map_err(|_| None)? {
        return Ok(String::new());
    }
    Ok(match user.received(&*conn, &searcher, act) {
        Ok(_) => String::new(),
        Err(e) => {
            println!("User inbox error: {}\n{}", e.as_fail(), e.backtrace());
            format!("Error: {}", e.as_fail())
        }
    })
}

#[get("/@/<name>/followers", rank = 1)]
pub fn ap_followers(
    name: String,
    conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_local(&*conn, &name).ok()?;
    let followers = user
        .get_followers(&*conn).ok()?
        .into_iter()
        .map(|f| Id::new(f.ap_url))
        .collect::<Vec<Id>>();

    let mut coll = OrderedCollection::default();
    coll.object_props
        .set_id_string(user.followers_endpoint).ok()?;
    coll.collection_props
        .set_total_items_u64(followers.len() as u64).ok()?;
    coll.collection_props
        .set_items_link_vec(followers).ok()?;
    Some(ActivityStream::new(coll))
}

#[get("/@/<name>/atom.xml")]
pub fn atom_feed(name: String, conn: DbConn) -> Option<Content<String>> {
    let author = User::find_by_fqn(&*conn, &name).ok()?;
    let feed = FeedBuilder::default()
        .title(author.display_name.clone())
        .id(Instance::get_local(&*conn)
            .unwrap()
            .compute_box("~", &name, "atom.xml"))
        .entries(
            Post::get_recents_for_author(&*conn, &author, 15).ok()?
                .into_iter()
                .map(|p| super::post_to_atom(p, &*conn))
                .collect::<Vec<Entry>>(),
        )
        .build()
        .expect("user::atom_feed: Error building Atom feed");
    Some(Content(
        ContentType::new("application", "atom+xml"),
        feed.to_string(),
    ))
}
