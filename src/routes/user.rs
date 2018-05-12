use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;
use serde_json;

use activity_pub::{activity, activity_pub, ActivityPub, context};
use activity_pub::actor::Actor;
use activity_pub::inbox::Inbox;
use activity_pub::outbox::Outbox;
use db_conn::DbConn;
use models::follows::*;
use models::instance::Instance;
use models::posts::Post;
use models::users::*;

#[get("/me")]
fn me(user: User) -> Redirect {
    Redirect::to(format!("/@/{}", user.username).as_ref())
}

#[get("/@/<name>", rank = 2)]
fn details(name: String, conn: DbConn, account: Option<User>) -> Template {
    let user = User::find_by_fqn(&*conn, name).unwrap();
    let recents = Post::get_recents_for_author(&*conn, &user, 5);
    let user_id = user.id.clone();

    Template::render("users/details", json!({
        "user": serde_json::to_value(user).unwrap(),
        "account": account,
        "recents": recents.into_iter().map(|p| {
            json!({
                "post": p,
                "author": p.get_authors(&*conn)[0],
                "url": format!("/~/{}/{}/", p.get_blog(&*conn).actor_id, p.slug),
                "date": p.creation_date.timestamp()
            })
        }).collect::<Vec<serde_json::Value>>(),
        "is_self": account.map(|a| a.id == user_id).unwrap_or(false)
    }))
}

#[get("/@/<name>/follow")]
fn follow(name: String, conn: DbConn, user: User) -> Redirect {
    let target = User::find_by_fqn(&*conn, name.clone()).unwrap();
    Follow::insert(&*conn, NewFollow {
        follower_id: user.id,
        following_id: target.id
    });
    target.send_to_inbox(&*conn, &user, activity::Follow::new(&user, &target, &*conn));
    Redirect::to(format!("/@/{}", name).as_ref())
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
fn create(conn: DbConn, data: Form<NewUserForm>) -> Redirect {
    let inst = Instance::get_local(&*conn).unwrap();
    let form = data.get();

    if form.password == form.password_confirmation {
        User::insert(&*conn, NewUser::new_local(
            form.username.to_string(),
            form.username.to_string(),
            !inst.has_admin(&*conn),
            String::from(""),
            form.email.to_string(),
            User::hash_pass(form.password.to_string()),
            inst.id
        )).update_boxes(&*conn);
    }
    
    Redirect::to(format!("/@/{}", data.get().username).as_str())
}

#[get("/@/<name>/outbox")]
fn outbox(name: String, conn: DbConn) -> Outbox {
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

#[get("/@/<name>/followers")]
fn followers(name: String, conn: DbConn) -> ActivityPub {
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
