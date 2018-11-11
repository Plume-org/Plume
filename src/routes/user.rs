use activitypub::{
    activity::Create,
    collection::OrderedCollection,
    object::Article
};
use atom_syndication::{Entry, FeedBuilder};
use rocket::{
    request::LenientForm,
    response::{Content, Flash, Redirect, status},
    http::{ContentType, Cookies}
};
use rocket_contrib::Template;
use serde_json;
use validator::{Validate, ValidationError};
use workerpool::thunk::*;

use plume_common::activity_pub::{
    ActivityStream, broadcast, Id, IntoId, ApRequest,
    inbox::{FromActivity, Notify, Deletable},
    sign::{Signable, verify_http_headers}
};
use plume_common::utils;
use plume_models::{
    blogs::Blog,
    db_conn::DbConn,
    follows,
    headers::Headers,
    instance::Instance,
    posts::Post,
    reshares::Reshare,
    users::*
};
use inbox::Inbox;
use routes::Page;
use Worker;

#[get("/me")]
fn me(user: Option<User>) -> Result<Redirect, Flash<Redirect>> {
    match user {
        Some(user) => Ok(Redirect::to(uri!(details: name = user.username))),
        None => Err(utils::requires_login("", uri!(me).into()))
    }
}

#[get("/@/<name>", rank = 2)]
fn details(name: String, conn: DbConn, account: Option<User>, worker: Worker, fecth_articles_conn: DbConn, fecth_followers_conn: DbConn, update_conn: DbConn) -> Template {
    may_fail!(account.map(|a| a.to_json(&*conn)), User::find_by_fqn(&*conn, name), "Couldn't find requested user", |user| {
        let recents = Post::get_recents_for_author(&*conn, &user, 6);
        let reshares = Reshare::get_recents_for_author(&*conn, &user, 6);
        let user_id = user.id.clone();
        let n_followers = user.get_followers(&*conn).len();

        if !user.get_instance(&*conn).local {
            // Fetch new articles
            let user_clone = user.clone();
            worker.execute(Thunk::of(move || {
                for create_act in user_clone.fetch_outbox::<Create>() {
                    match create_act.create_props.object_object::<Article>() {
                        Ok(article) => {
                            Post::from_activity(&*fecth_articles_conn, article, user_clone.clone().into_id());
                            println!("Fetched article from remote user");
                        }
                        Err(e) => println!("Error while fetching articles in background: {:?}", e)
                    }
                }
            }));

            // Fetch followers
            let user_clone = user.clone();
            worker.execute(Thunk::of(move || {
                for user_id in user_clone.fetch_followers_ids() {
                    let follower = User::find_by_ap_url(&*fecth_followers_conn, user_id.clone())
                        .unwrap_or_else(|| User::fetch_from_url(&*fecth_followers_conn, user_id).expect("user::details: Couldn't fetch follower"));
                    follows::Follow::insert(&*fecth_followers_conn, follows::NewFollow {
                        follower_id: follower.id,
                        following_id: user_clone.id,
                        ap_url: format!("{}/follow/{}", follower.ap_url, user_clone.ap_url),
                    });
                }
            }));

            // Update profile information if needed
            let user_clone = user.clone();
            if user.needs_update() {
                worker.execute(Thunk::of(move || {
                    user_clone.refetch(&*update_conn);
                }))
            }
        }

        Template::render("users/details", json!({
            "user": user.to_json(&*conn),
            "instance_url": user.get_instance(&*conn).public_domain,
            "is_remote": user.instance_id != Instance::local_id(&*conn),
            "follows": account.clone().map(|x| x.is_following(&*conn, user.id)).unwrap_or(false),
            "account": account.clone().map(|a| a.to_json(&*conn)),
            "recents": recents.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "reshares": reshares.into_iter().map(|r| r.get_post(&*conn).unwrap().to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "is_self": account.map(|a| a.id == user_id).unwrap_or(false),
            "n_followers": n_followers
        }))
    })
}

#[get("/dashboard")]
fn dashboard(user: User, conn: DbConn) -> Template {
    let blogs = Blog::find_for_author(&*conn, &user);
    Template::render("users/dashboard", json!({
        "account": user.to_json(&*conn),
        "blogs": blogs,
        "drafts": Post::drafts_by_author(&*conn, &user).into_iter().map(|a| a.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
    }))
}

#[get("/dashboard", rank = 2)]
fn dashboard_auth() -> Flash<Redirect> {
    utils::requires_login(
        "You need to be logged in order to access your dashboard",
        uri!(dashboard).into()
    )
}

#[post("/@/<name>/follow")]
fn follow(name: String, conn: DbConn, user: User, worker: Worker) -> Option<Redirect> {
    let target = User::find_by_fqn(&*conn, name.clone())?;
    if let Some(follow) = follows::Follow::find(&*conn, user.id, target.id) {
        let delete_act = follow.delete(&*conn);
        worker.execute(Thunk::of(move || broadcast(&user, delete_act, vec![target])));
    } else {
        let f = follows::Follow::insert(&*conn, follows::NewFollow {
            follower_id: user.id,
            following_id: target.id,
            ap_url: format!("{}/follow/{}", user.ap_url, target.ap_url),
        });
        f.notify(&*conn);

        let act = f.into_activity(&*conn);
        worker.execute(Thunk::of(move || broadcast(&user, act, vec![target])));
    }
    Some(Redirect::to(uri!(details: name = name)))
}

#[post("/@/<name>/follow", rank = 2)]
fn follow_auth(name: String) -> Flash<Redirect> {
    utils::requires_login(
        "You need to be logged in order to follow someone",
        uri!(follow: name = name).into()
    )
}

#[get("/@/<name>/followers?<page>")]
fn followers_paginated(name: String, conn: DbConn, account: Option<User>, page: Page) -> Template {
    may_fail!(account.map(|a| a.to_json(&*conn)), User::find_by_fqn(&*conn, name.clone()), "Couldn't find requested user", |user| {
        let user_id = user.id.clone();
        let followers_count = user.get_followers(&*conn).len();

        Template::render("users/followers", json!({
            "user": user.to_json(&*conn),
            "instance_url": user.get_instance(&*conn).public_domain,
            "is_remote": user.instance_id != Instance::local_id(&*conn),
            "follows": account.clone().map(|x| x.is_following(&*conn, user.id)).unwrap_or(false),
            "followers": user.get_followers_page(&*conn, page.limits()).into_iter().map(|f| f.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "account": account.clone().map(|a| a.to_json(&*conn)),
            "is_self": account.map(|a| a.id == user_id).unwrap_or(false),
            "n_followers": followers_count,
            "page": page.page,
            "n_pages": Page::total(followers_count as i32)
        }))
    })
}

#[get("/@/<name>/followers", rank = 2)]
fn followers(name: String, conn: DbConn, account: Option<User>) -> Template {
    followers_paginated(name, conn, account, Page::first())
}


#[get("/@/<name>", rank = 1)]
fn activity_details(name: String, conn: DbConn, _ap: ApRequest) -> Option<ActivityStream<CustomPerson>> {
    let user = User::find_local(&*conn, name)?;
    Some(ActivityStream::new(user.into_activity(&*conn)))
}

#[get("/users/new")]
fn new(user: Option<User>, conn: DbConn) -> Template {
    Template::render("users/new", json!({
        "enabled": Instance::get_local(&*conn).map(|i| i.open_registrations).unwrap_or(true),
        "account": user.map(|u| u.to_json(&*conn)),
        "errors": null,
        "form": null
    }))
}

#[get("/@/<name>/edit")]
fn edit(name: String, user: User, conn: DbConn) -> Option<Template> {
    if user.username == name && !name.contains("@") {
        Some(Template::render("users/edit", json!({
            "account": user.to_json(&*conn)
        })))
    } else {
        None
    }
}

#[get("/@/<name>/edit", rank = 2)]
fn edit_auth(name: String) -> Flash<Redirect> {
    utils::requires_login(
        "You need to be logged in order to edit your profile",
        uri!(edit: name = name).into()
    )
}

#[derive(FromForm)]
struct UpdateUserForm {
    display_name: Option<String>,
    email: Option<String>,
    summary: Option<String>,
}

#[put("/@/<_name>/edit", data = "<data>")]
fn update(_name: String, conn: DbConn, user: User, data: LenientForm<UpdateUserForm>) -> Redirect {
    user.update(&*conn,
        data.get().display_name.clone().unwrap_or(user.display_name.to_string()).to_string(),
        data.get().email.clone().unwrap_or(user.email.clone().unwrap()).to_string(),
        data.get().summary.clone().unwrap_or(user.summary.to_string())
    );
    Redirect::to(uri!(me))
}

#[post("/@/<name>/delete")]
fn delete(name: String, conn: DbConn, user: User, mut cookies: Cookies) -> Option<Redirect> {
    let account = User::find_by_fqn(&*conn, name.clone())?;
    if user.id == account.id {
        account.delete(&*conn);

        cookies.get_private(AUTH_COOKIE).map(|cookie| cookies.remove_private(cookie));

        Some(Redirect::to(uri!(super::instance::index)))
    } else {
        Some(Redirect::to(uri!(edit: name = name)))
    }
}

#[derive(FromForm, Serialize, Validate)]
#[validate(schema(function = "passwords_match", skip_on_field_errors = "false", message = "Passwords are not matching"))]
struct NewUserForm {
    #[validate(length(min = "1", message = "Username can't be empty"))]
    username: String,
    #[validate(email(message = "Invalid email"))]
    email: String,
    #[validate(length(min = "8", message = "Password should be at least 8 characters long"))]
    password: String,
    #[validate(length(min = "8", message = "Password should be at least 8 characters long"))]
    password_confirmation: String
}

fn passwords_match(form: &NewUserForm) -> Result<(), ValidationError> {
    if form.password != form.password_confirmation {
        Err(ValidationError::new("password_match"))
    } else {
        Ok(())
    }
}

#[post("/users/new", data = "<data>")]
fn create(conn: DbConn, data: LenientForm<NewUserForm>) -> Result<Redirect, Template> {
    if !Instance::get_local(&*conn).map(|i| i.open_registrations).unwrap_or(true) {
        return Ok(Redirect::to(uri!(new))); // Actually, it is an error
    }

    let form = data.get();
    form.validate()
        .map(|_| {
             NewUser::new_local(
                &*conn,
                form.username.to_string(),
                form.username.to_string(),
                false,
                String::from(""),
                form.email.to_string(),
                User::hash_pass(form.password.to_string())
            ).update_boxes(&*conn);
            Redirect::to(uri!(super::session::new))
        })
        .map_err(|e| Template::render("users/new", json!({
            "enabled": Instance::get_local(&*conn).map(|i| i.open_registrations).unwrap_or(true),
            "errors": e.inner(),
            "form": form
        })))
}

#[get("/@/<name>/outbox")]
fn outbox(name: String, conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_local(&*conn, name)?;
    Some(user.outbox(&*conn))
}

#[post("/@/<name>/inbox", data = "<data>")]
fn inbox(name: String, conn: DbConn, data: String, headers: Headers) -> Result<String, Option<status::BadRequest<&'static str>>> {
    let user = User::find_local(&*conn, name).ok_or(None)?;
    let act: serde_json::Value = serde_json::from_str(&data[..]).expect("user::inbox: deserialization error");

    let activity = act.clone();
    let actor_id = activity["actor"].as_str()
        .or_else(|| activity["actor"]["id"].as_str()).ok_or(Some(status::BadRequest(Some("Missing actor id for activity"))))?;

    let actor = User::from_url(&conn, actor_id.to_owned()).expect("user::inbox: user error");
    if !verify_http_headers(&actor, headers.0.clone(), data).is_secure() &&
        !act.clone().verify(&actor) {
        println!("Rejected invalid activity supposedly from {}, with headers {:?}", actor.username, headers.0);
        return Err(Some(status::BadRequest(Some("Invalid signature"))));
    }

    if Instance::is_blocked(&*conn, actor_id.to_string()) {
        return Ok(String::new());
    }
    Ok(match user.received(&*conn, act) {
        Ok(_) => String::new(),
        Err(e) => {
            println!("User inbox error: {}\n{}", e.as_fail(), e.backtrace());
            format!("Error: {}", e.as_fail())
        }
    })
}

#[get("/@/<name>/followers")]
fn ap_followers(name: String, conn: DbConn, _ap: ApRequest) -> Option<ActivityStream<OrderedCollection>> {
    let user = User::find_local(&*conn, name)?;
    let followers = user.get_followers(&*conn).into_iter().map(|f| Id::new(f.ap_url)).collect::<Vec<Id>>();

    let mut coll = OrderedCollection::default();
    coll.object_props.set_id_string(user.followers_endpoint).expect("user::ap_followers: id error");
    coll.collection_props.set_total_items_u64(followers.len() as u64).expect("user::ap_followers: totalItems error");
    coll.collection_props.set_items_link_vec(followers).expect("user::ap_followers items error");
    Some(ActivityStream::new(coll))
}

#[get("/@/<name>/atom.xml")]
fn atom_feed(name: String, conn: DbConn) -> Option<Content<String>> {
    let author = User::find_by_fqn(&*conn, name.clone())?;
    let feed = FeedBuilder::default()
        .title(author.display_name.clone())
        .id(Instance::get_local(&*conn).unwrap().compute_box("~", name, "atom.xml"))
        .entries(Post::get_recents_for_author(&*conn, &author, 15)
            .into_iter()
            .map(|p| super::post_to_atom(p, &*conn))
            .collect::<Vec<Entry>>())
        .build()
        .expect("user::atom_feed: Error building Atom feed");
    Some(Content(ContentType::new("application", "atom+xml"), feed.to_string()))
}
