use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;
use std::collections::HashMap;

use activity_pub::Activity;
use activity_pub::actor::Actor;
use db_conn::DbConn;
use models::blog_authors::*;
use models::blogs::*;
use models::instance::Instance;
use models::users::User;
use utils;

#[get("/~/<name>", rank = 2)]
fn details(name: String) -> String {
    format!("Welcome on ~{}", name)
}

#[get("/~/<name>", format = "application/activity+json", rank = 1)]
fn activity_details(name: String, conn: DbConn) -> Activity {
    let blog = Blog::find_by_actor_id(&*conn, name).unwrap();
    blog.as_activity_pub(&*conn)
}

#[get("/blogs/new")]
fn new(_user: User) -> Template {
    Template::render("blogs/new", HashMap::<String, i32>::new())
}

#[derive(FromForm)]
struct NewBlogForm {
    pub title: String
}

#[post("/blogs/new", data = "<data>")]
fn create(conn: DbConn, data: Form<NewBlogForm>, user: User) -> Redirect {
    let inst = Instance::get_local(&*conn).unwrap();
    let form = data.get();
    let slug = utils::make_actor_id(form.title.to_string());

    let blog = Blog::insert(&*conn, NewBlog::new_local(
        slug.to_string(),
        form.title.to_string(),
        String::from(""),
        inst.id
    ));
    blog.update_boxes(&*conn);

    BlogAuthor::insert(&*conn, NewBlogAuthor {
        blog_id: blog.id,
        author_id: user.id,
        is_owner: true
    });
    
    Redirect::to(format!("/~/{}", slug).as_str())
}
