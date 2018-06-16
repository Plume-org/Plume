use activitypub::collection::OrderedCollection;
use rocket::{
    request::Form,
    response::{Redirect, Flash}
};
use rocket_contrib::Template;
use serde_json;

use activity_pub::{ActivityStream, ActivityPub, actor::Actor};
use db_conn::DbConn;
use models::{
    blog_authors::*,
    blogs::*,
    instance::Instance,
    posts::Post,
    users::User
};
use utils;

#[get("/~/<name>", rank = 2)]
fn details(name: String, conn: DbConn, user: Option<User>) -> Template {
    let blog = Blog::find_by_fqn(&*conn, name).unwrap();
    let recents = Post::get_recents_for_blog(&*conn, &blog, 5);
    Template::render("blogs/details", json!({
        "blog": blog,
        "account": user,
        "is_author": user.map(|x| x.is_author_in(&*conn, blog)),
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
        }).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/~/<name>", format = "application/activity+json", rank = 1)]
fn activity_details(name: String, conn: DbConn) -> ActivityPub {
    let blog = Blog::find_local(&*conn, name).unwrap();
    blog.as_activity_pub(&*conn)
}

#[get("/blogs/new")]
fn new(user: User) -> Template {
    Template::render("blogs/new", json!({
        "account": user
    }))
}

#[get("/blogs/new", rank = 2)]
fn new_auth() -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to create a new blog", "/blogs/new")
}

#[derive(FromForm)]
struct NewBlogForm {
    pub title: String
}

#[post("/blogs/new", data = "<data>")]
fn create(conn: DbConn, data: Form<NewBlogForm>, user: User) -> Redirect {
    let form = data.get();
    let slug = utils::make_actor_id(form.title.to_string());

    let blog = Blog::insert(&*conn, NewBlog::new_local(
        slug.to_string(),
        form.title.to_string(),
        String::from(""),
        Instance::local_id(&*conn)
    ));
    blog.update_boxes(&*conn);

    BlogAuthor::insert(&*conn, NewBlogAuthor {
        blog_id: blog.id,
        author_id: user.id,
        is_owner: true
    });
    
    Redirect::to(format!("/~/{}/", slug))
}

#[get("/~/<name>/outbox")]
fn outbox(name: String, conn: DbConn) -> ActivityStream<OrderedCollection> {
    let blog = Blog::find_local(&*conn, name).unwrap();
    blog.outbox(&*conn)
}
