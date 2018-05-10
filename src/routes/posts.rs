use heck::KebabCase;
use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;
use serde_json;
use std::collections::HashMap;

use activity_pub::{context, activity_pub, ActivityPub};
use activity_pub::activity::Create;
use activity_pub::object::Object;
use activity_pub::outbox::broadcast;
use db_conn::DbConn;
use models::blogs::*;
use models::comments::Comment;
use models::post_authors::*;
use models::posts::*;
use models::users::User;
use utils;

#[get("/~/<blog>/<slug>", rank = 4)]
fn details(blog: String, slug: String, conn: DbConn) -> Template {
    let blog = Blog::find_by_actor_id(&*conn, blog).unwrap();
    let post = Post::find_by_slug(&*conn, slug).unwrap();
    let comments = Comment::for_post(&*conn, post.id);    
    Template::render("posts/details", json!({
        "post": post,
        "blog": blog,
        "comments": comments.into_iter().map(|c| {
            json!({
                "content": c.content,
                "author": c.get_author(&*conn)
            })
        }).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/~/<_blog>/<slug>", rank = 3, format = "application/activity+json")]
fn activity_details(_blog: String, slug: String, conn: DbConn) -> ActivityPub {
    // TODO: posts in different blogs may have the same slug
    let post = Post::find_by_slug(&*conn, slug).unwrap();

    let mut act = post.serialize(&*conn);
    act["@context"] = context();
    activity_pub(act)
}

#[get("/~/<_blog>/new", rank = 2)]
fn new_auth(_blog: String) -> Redirect {
    utils::requires_login()
}

#[get("/~/<_blog>/new", rank = 1)]
fn new(_blog: String, _user: User) -> Template {
    Template::render("posts/new", HashMap::<String, String>::new())
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
        license: form.license.to_string(),
        ap_url: "".to_string()
    });
    post.update_ap_url(&*conn);
    PostAuthor::insert(&*conn, NewPostAuthor {
        post_id: post.id,
        author_id: user.id
    });

    let act = Create::new(&user, &post, &*conn);
    broadcast(&*conn, &user, act, user.get_followers(&*conn));

    Redirect::to(format!("/~/{}/{}", blog_name, slug).as_str())
}
