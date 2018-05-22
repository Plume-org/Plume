use heck::KebabCase;
use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;
use serde_json;

use activity_pub::{broadcast, context, activity_pub, ActivityPub, object::Object};
use db_conn::DbConn;
use models::{
    blogs::*,
    comments::Comment,
    post_authors::*,
    posts::*,
    users::User
};
use utils;

#[get("/~/<blog>/<slug>", rank = 4)]
fn details(blog: String, slug: String, conn: DbConn, user: Option<User>) -> Template {
    let blog = Blog::find_by_fqn(&*conn, blog).unwrap();
    let post = Post::find_by_slug(&*conn, slug).unwrap();
    let comments = Comment::find_by_post(&*conn, post.id);

    Template::render("posts/details", json!({
        "author": ({
            let author = &post.get_authors(&*conn)[0];
            let mut json = serde_json::to_value(author).unwrap();
            json["fqn"] = serde_json::Value::String(author.get_fqn(&*conn));
            json
        }),
        "post": post,
        "blog": blog,
        "comments": comments.into_iter().map(|c| {
            json!({
                "id": c.id,
                "content": c.content,
                "author": c.get_author(&*conn)
            })
        }).collect::<Vec<serde_json::Value>>(),
        "n_likes": post.get_likes(&*conn).len(),
        "has_liked": user.clone().map(|u| u.has_liked(&*conn, &post)).unwrap_or(false),
        "account": user,
        "date": &post.creation_date.timestamp()
    }))
}

#[get("/~/<_blog>/<slug>", rank = 3, format = "application/activity+json")]
fn activity_details(_blog: String, slug: String, conn: DbConn) -> ActivityPub {
    // FIXME: posts in different blogs may have the same slug
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
fn new(_blog: String, user: User) -> Template {
    Template::render("posts/new", json!({
        "account": user
    }))
}

#[derive(FromForm)]
struct NewPostForm {
    pub title: String,
    pub content: String,
    pub license: String
}

#[post("/~/<blog_name>/new", data = "<data>")]
fn create(blog_name: String, data: Form<NewPostForm>, user: User, conn: DbConn) -> Redirect {
    let blog = Blog::find_by_fqn(&*conn, blog_name.to_string()).unwrap();
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

    let act = post.create_activity(&*conn);
    broadcast(&*conn, &user, act, user.get_followers(&*conn));

    Redirect::to(format!("/~/{}/{}/", blog_name, slug).as_str())
}
