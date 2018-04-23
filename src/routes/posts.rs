use rocket::response::Redirect;
use rocket::request::Form;
use rocket_contrib::Template;
use std::collections::HashMap;
use heck::KebabCase;

use utils;
use db_conn::DbConn;
use models::blogs::*;
use models::post::*;
use models::post_authors::*;
use models::user::User;

#[get("/~/<blog>/<slug>", rank = 3)]
fn details(blog: String, slug: String, conn: DbConn) -> String {
    let blog = Blog::find_by_actor_id(&*conn, blog).unwrap();
    let post = Post::find_by_slug(&*conn, slug).unwrap();
    format!("{} in {}", post.title, blog.title)
}

#[get("/~/<blog>/new", rank = 1)]
fn new(blog: String, user: User) -> Template {
    Template::render("posts/new", HashMap::<String, String>::new())
}

#[get("/~/<blog>/new", rank = 2)]
fn new_auth(blog: String) -> Redirect {
    utils::requires_login()
}

#[derive(FromForm)]
struct NewPostForm {
    pub title: String,
    pub content: String,
    pub license: String
}

#[post("/~/<blog_name>/new", data = "<data>")]
fn create(blog_name: String, data: Form<NewPostForm>, user: User, conn: DbConn) -> Redirect {
    let blog = Blog::find_by_actor_id(&*conn, blog_name.to_string()).unwrap();
    let form = data.get();
    let slug = form.title.to_string().to_kebab_case();
    let post = Post::insert(&*conn, NewPost {
        blog_id: blog.id,
        slug: slug.to_string(),
        title: form.title.to_string(),
        content: form.content.to_string(),
        published: true,
        license: form.license.to_string()
    });
    PostAuthor::insert(&*conn, NewPostAuthor {
        post_id: post.id,
        author_id: user.id
    });
    Redirect::to(format!("/~/{}/{}", blog_name, slug).as_str())
}
