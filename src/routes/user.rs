use activitypub::{
    activity::Follow,
    collection::OrderedCollection
};
use rocket::{request::Form,
    response::{Redirect, Flash}
};
use rocket_contrib::Template;
use serde_json;

use activity_pub::{
    activity_pub, ActivityPub, ActivityStream, context, broadcast, Id, IntoId,
    inbox::{Inbox, Notify},
    actor::Actor
};
use db_conn::DbConn;
use models::{
    blogs::Blog,
    follows,
    instance::Instance,
    posts::Post,
    reshares::Reshare,
    users::*
};
use utils;

#[get("/me")]
fn me(user: Option<User>) -> Result<Redirect, Flash<Redirect>> {
    match user {
        Some(user) => Ok(Redirect::to(uri!(details: name = user.username))),
        None => Err(utils::requires_login("", uri!(me)))
    }
}

#[get("/@/<name>", rank = 2)]
fn details(name: String, conn: DbConn, account: Option<User>) -> Template {
    may_fail!(User::find_by_fqn(&*conn, name), "Couldn't find requested user", |user| {
        let recents = Post::get_recents_for_author(&*conn, &user, 6);
        let reshares = Reshare::get_recents_for_author(&*conn, &user, 6);
        let user_id = user.id.clone();
        let n_followers = user.get_followers(&*conn).len();

        Template::render("users/details", json!({
            "user": serde_json::to_value(user.clone()).unwrap(),
            "instance_url": user.get_instance(&*conn).public_domain,
            "is_remote": user.instance_id != Instance::local_id(&*conn),
            "follows": account.clone().map(|x| x.is_following(&*conn, user.id)).unwrap_or(false),
            "account": account,
            "recents": recents.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "reshares": reshares.into_iter().map(|r| r.get_post(&*conn).unwrap().to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "is_self": account.map(|a| a.id == user_id).unwrap_or(false),
            "n_followers": n_followers
        }))
    })
}

#[get("/dashboard")]
fn dashboard(user: User, conn: DbConn) -> Template {
    let blogs = Blog::find_for_author(&*conn, user.id);
    Template::render("users/dashboard", json!({
        "account": user,
        "blogs": blogs
    }))
}

#[get("/dashboard", rank = 2)]
fn dashboard_auth() -> Flash<Redirect> {
    utils::requires_login("You need to be logged in order to access your dashboard", uri!(dashboard))
}

#[get("/@/<name>/follow")]
fn follow(name: String, conn: DbConn, user: User) -> Redirect {
    let target = User::find_by_fqn(&*conn, name.clone()).unwrap();
    let f = follows::Follow::insert(&*conn, follows::NewFollow {
        follower_id: user.id,
        following_id: target.id
    });
    f.notify(&*conn);

    let mut act = Follow::default();
    act.follow_props.set_actor_link::<Id>(user.clone().into_id()).unwrap();
    act.follow_props.set_object_object(user.into_activity(&*conn)).unwrap();
    act.object_props.set_id_string(format!("{}/follow/{}", user.ap_url, target.ap_url)).unwrap();

    broadcast(&*conn, &user, act, vec![target]);
    Redirect::to(uri!(details: name = name))
}

#[get("/@/<name>/follow", rank = 2)]
fn follow_auth(name: String) -> Flash<Redirect> {
    utils::requires_login("You need to be logged in order to follow someone", uri!(follow: name = name))
}

#[get("/@/<name>/followers", rank = 2)]
fn followers(name: String, conn: DbConn, account: Option<User>) -> Template {
    may_fail!(User::find_by_fqn(&*conn, name.clone()), "Couldn't find requested user", |user| {
        let user_id = user.id.clone();

        Template::render("users/followers", json!({
            "user": serde_json::to_value(user.clone()).unwrap(),
            "instance_url": user.get_instance(&*conn).public_domain,
            "is_remote": user.instance_id != Instance::local_id(&*conn),
            "follows": account.clone().map(|x| x.is_following(&*conn, user.id)).unwrap_or(false),
            "followers": user.get_followers(&*conn).into_iter().map(|f| f.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "account": account,
            "is_self": account.map(|a| a.id == user_id).unwrap_or(false),
            "n_followers": user.get_followers(&*conn).len()
        }))
    })
}

#[get("/@/<name>", format = "application/activity+json", rank = 1)]
fn activity_details(name: String, conn: DbConn) -> ActivityPub {
    let user = User::find_local(&*conn, name).unwrap();
    user.as_activity_pub(&*conn)
}

#[get("/users/new")]
fn new(user: Option<User>) -> Template {
    Template::render("users/new", json!({
        "account": user
    }))
}

#[get("/@/<name>/edit")]
fn edit(name: String, user: User) -> Option<Template> {
    if user.username == name && !name.contains("@") {
        Some(Template::render("users/edit", json!({
            "account": user
        })))
    } else {
        None
    }
}

#[get("/@/<name>/edit", rank = 2)]
fn edit_auth(name: String) -> Flash<Redirect> {
    utils::requires_login("You need to be logged in order to edit your profile", uri!(edit: name = name))
}

#[derive(FromForm)]
struct UpdateUserForm {
    display_name: Option<String>,
    email: Option<String>,
    summary: Option<String>,
}

#[put("/@/<_name>/edit", data = "<data>")]
fn update(_name: String, conn: DbConn, user: User, data: Form<UpdateUserForm>) -> Redirect {
    user.update(&*conn,
        data.get().display_name.clone().unwrap_or(user.display_name.to_string()).to_string(),
        data.get().email.clone().unwrap_or(user.email.clone().unwrap()).to_string(),
        data.get().summary.clone().unwrap_or(user.summary.to_string())
    );
    Redirect::to(uri!(me))
}

#[derive(FromForm)]
struct NewUserForm {
    username: String,
    email: String,
    password: String,
    password_confirmation: String
}

#[post("/users/new", data = "<data>")]
fn create(conn: DbConn, data: Form<NewUserForm>) -> Result<Redirect, String> {
    let form = data.get();

    if form.username.clone().len() < 1 {
        Err(String::from("Username is required"))
    } else if form.email.clone().len() < 1 {
        Err(String::from("Email is required"))
    } else if form.password.clone().len() < 8 {
        Err(String::from("Password should be at least 8 characters long"))
    } else if form.password == form.password_confirmation {
        NewUser::new_local(
            &*conn,
            form.username.to_string(),
            form.username.to_string(),
            false,
            String::from(""),
            form.email.to_string(),
            User::hash_pass(form.password.to_string())
        ).update_boxes(&*conn);
        Ok(Redirect::to(uri!(super::session::new)))
    } else {
        Err(String::from("Passwords don't match"))
    }
}

#[get("/@/<name>/outbox")]
fn outbox(name: String, conn: DbConn) -> ActivityStream<OrderedCollection> {
    let user = User::find_local(&*conn, name).unwrap();
    user.outbox(&*conn)
}

#[post("/@/<name>/inbox", data = "<data>")]
fn inbox(name: String, conn: DbConn, data: String) -> String {
    let user = User::find_local(&*conn, name).unwrap();
    let act: serde_json::Value = serde_json::from_str(&data[..]).unwrap();
    user.received(&*conn, act);
    String::from("")
}

#[get("/@/<name>/followers", format = "application/activity+json")]
fn ap_followers(name: String, conn: DbConn) -> ActivityPub {
    let user = User::find_local(&*conn, name).unwrap();
    let followers = user.get_followers(&*conn).into_iter().map(|f| f.compute_id(&*conn)).collect::<Vec<String>>();
    
    let json = json!({
        "@context": context(),
        "id": user.compute_box(&*conn, "followers"),
        "type": "OrderedCollection",
        "totalItems": followers.len(),
        "orderedItems": followers
    });
    activity_pub(json)
}
