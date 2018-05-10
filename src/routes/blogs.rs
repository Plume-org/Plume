use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;
use std::collections::HashMap;

use activity_pub::ActivityPub;
use activity_pub::actor::Actor;
use activity_pub::outbox::Outbox;
use db_conn::DbConn;
use models::blog_authors::*;
use models::blogs::*;
use models::instance::Instance;
use models::users::User;
use utils;

#[get("/~/<name>", rank = 2)]
fn details(name: String, conn: DbConn, user: Option<User>) -> Template {
    let blog = Blog::find_by_actor_id(&*conn, name).unwrap();    
    Template::render("blogs/details", json!({
        "blog": blog,
        "account": user
    }))
}

#[get("/~/<name>", format = "application/activity+json", rank = 1)]
fn activity_details(name: String, conn: DbConn) -> ActivityPub {
    let blog = Blog::find_by_actor_id(&*conn, name).unwrap();
    blog.as_activity_pub(&*conn)
}

#[get("/blogs/new")]
fn new(user: User) -> Template {
    Template::render("blogs/new", json!({
        "account": user
    }))
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
    
    Redirect::to(format!("/~/{}", slug).as_str())
}

#[get("/~/<name>/outbox")]
fn outbox(name: String, conn: DbConn) -> Outbox {
    let blog = Blog::find_by_actor_id(&*conn, name).unwrap();
    blog.outbox(&*conn)
}
