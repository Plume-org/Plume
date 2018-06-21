use activitypub::collection::OrderedCollection;
use rocket::{
    request::Form,
    response::{Redirect, Flash}
};
use rocket_contrib::Template;
use serde_json;

use activity_pub::ActivityStream;
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
    may_fail!(user, Blog::find_by_fqn(&*conn, name), "Requested blog couldn't be found", |blog| {
        let recents = Post::get_recents_for_blog(&*conn, &blog, 5);

        Template::render("blogs/details", json!({
            "blog": blog,
            "account": user,
            "is_author": user.map(|x| x.is_author_in(&*conn, blog)),
            "recents": recents.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
        }))
    })    
}

#[get("/~/<name>", format = "application/activity+json", rank = 1)]
fn activity_details(name: String, conn: DbConn) -> ActivityStream<CustomGroup> {
    let blog = Blog::find_local(&*conn, name).unwrap();
    ActivityStream::new(blog.into_activity(&*conn))
}

#[get("/blogs/new")]
fn new(user: User) -> Template {
    Template::render("blogs/new", json!({
        "account": user
    }))
}

#[get("/blogs/new", rank = 2)]
fn new_auth() -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to create a new blog", uri!(new))
}

#[derive(FromForm)]
struct NewBlogForm {
    pub title: String
}

#[post("/blogs/new", data = "<data>")]
fn create(conn: DbConn, data: Form<NewBlogForm>, user: User) -> Redirect {
    let form = data.get();
    let slug = utils::make_actor_id(form.title.to_string());

    if Blog::find_local(&*conn, slug.clone()).is_some() || slug.len() == 0 {
        Redirect::to(uri!(new))
    } else {
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

        Redirect::to(uri!(details: name = slug))
    }
}

#[get("/~/<name>/outbox")]
fn outbox(name: String, conn: DbConn) -> ActivityStream<OrderedCollection> {
    let blog = Blog::find_local(&*conn, name).unwrap();
    blog.outbox(&*conn)
}
