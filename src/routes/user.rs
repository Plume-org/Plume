use activitystreams_types::{
    activity::Follow,
    collection::OrderedCollection
};
use rocket::{request::Form, response::Redirect};
use rocket_contrib::Template;
use serde_json;

use activity_pub::{
    activity_pub, ActivityPub, ActivityStream, context, broadcast, Id, IntoId,
    inbox::Inbox,
    actor::Actor
};
use db_conn::DbConn;
use models::{
    follows,
    instance::Instance,
    posts::Post,
    users::*
};

#[get("/me")]
fn me(user: User) -> Redirect {
    Redirect::to(format!("/@/{}/", user.username).as_ref())
}

#[get("/@/<name>", rank = 2)]
fn details(name: String, conn: DbConn, account: Option<User>) -> Template {
    let user = User::find_by_fqn(&*conn, name).unwrap();
    let recents = Post::get_recents_for_author(&*conn, &user, 5);
    let user_id = user.id.clone();
    let n_followers = user.get_followers(&*conn).len();

    Template::render("users/details", json!({
        "user": serde_json::to_value(user).unwrap(),
        "account": account,
        "recents": recents.into_iter().map(|p| {
            json!({
                "post": p,
                "author": ({
                    let author = &p.get_authors(&*conn)[0];
                    let mut json = serde_json::to_value(author).unwrap();
                    json["fqn"] = serde_json::Value::String(author.get_fqn(&*conn));
                    json
                }),
                "url": format!("/~/{}/{}/", p.get_blog(&*conn).actor_id, p.slug),
                "date": p.creation_date.timestamp()
            })
        }).collect::<Vec<serde_json::Value>>(),
        "is_self": account.map(|a| a.id == user_id).unwrap_or(false),
        "n_followers": n_followers
    }))
}

#[get("/@/<name>/follow")]
fn follow(name: String, conn: DbConn, user: User) -> Redirect {
    let target = User::find_by_fqn(&*conn, name.clone()).unwrap();
    follows::Follow::insert(&*conn, follows::NewFollow {
        follower_id: user.id,
        following_id: target.id
    });
    let mut act = Follow::default();
    act.set_actor_link::<Id>(user.clone().into_id()).unwrap();
    act.set_object_object(user.into_activity(&*conn)).unwrap();
    act.object_props.set_id_string(format!("{}/follow/{}", user.ap_url, target.ap_url)).unwrap();
    broadcast(&*conn, &user, act, vec![target]);
    Redirect::to(format!("/@/{}/", name).as_ref())
}

#[get("/@/<name>/followers", rank = 2)]
fn followers(name: String, conn: DbConn, account: Option<User>) -> Template {
    let user = User::find_by_fqn(&*conn, name.clone()).unwrap();
    let user_id = user.id.clone();
    
    Template::render("users/followers", json!({
        "user": serde_json::to_value(user.clone()).unwrap(),
        "followers": user.get_followers(&*conn).into_iter().map(|f| {
            let fqn = f.get_fqn(&*conn);
            let mut json = serde_json::to_value(f).unwrap();
            json["fqn"] = serde_json::Value::String(fqn);
            json
        }).collect::<Vec<serde_json::Value>>(),
        "account": account,
        "is_self": account.map(|a| a.id == user_id).unwrap_or(false)
    }))
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
    Redirect::to("/me")
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
    let inst = Instance::get_local(&*conn).unwrap();
    let form = data.get();

    if form.username.clone().len() < 1 {
        Err(String::from("Username is required"))
    } else if form.email.clone().len() < 1 {
        Err(String::from("Email is required"))
    } else if form.password.clone().len() < 8 {
        Err(String::from("Password should be at least 8 characters long"))
    } else if form.password == form.password_confirmation {
        User::insert(&*conn, NewUser::new_local(
            form.username.to_string(),
            form.username.to_string(),
            !inst.has_admin(&*conn),
            String::from(""),
            form.email.to_string(),
            User::hash_pass(form.password.to_string()),
            inst.id
        )).update_boxes(&*conn);
        Ok(Redirect::to(format!("/@/{}/", data.get().username).as_str()))
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
