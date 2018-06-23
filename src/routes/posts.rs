use activitypub::object::Article;
use heck::KebabCase;
use rocket::request::Form;
use rocket::response::{Redirect, Flash};
use rocket_contrib::Template;
use serde_json;

use plume_common::activity_pub::{broadcast, ActivityStream};
use plume_common::utils;
use plume_models::{
    blogs::*,
    db_conn::DbConn,
    comments::Comment,
    mentions::Mention,
    post_authors::*,
    posts::*,
    safe_string::SafeString,
    users::User
};
use routes::comments::CommentQuery;

// See: https://github.com/SergioBenitez/Rocket/pull/454
#[get("/~/<blog>/<slug>", rank = 4)]
fn details(blog: String, slug: String, conn: DbConn, user: Option<User>) -> Template {
    details_response(blog, slug, conn, user, None)
}

#[get("/~/<blog>/<slug>?<query>")]
fn details_response(blog: String, slug: String, conn: DbConn, user: Option<User>, query: Option<CommentQuery>) -> Template {
    may_fail!(user, Blog::find_by_fqn(&*conn, blog), "Couldn't find this blog", |blog| {
        may_fail!(user, Post::find_by_slug(&*conn, slug, blog.id), "Couldn't find this post", |post| {
            let comments = Comment::list_by_post(&*conn, post.id);

            Template::render("posts/details", json!({
                "author": post.get_authors(&*conn)[0].to_json(&*conn),
                "post": post,
                "blog": blog,
                "comments": comments.into_iter().map(|c| c.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
                "n_likes": post.get_likes(&*conn).len(),
                "has_liked": user.clone().map(|u| u.has_liked(&*conn, &post)).unwrap_or(false),
                "n_reshares": post.get_reshares(&*conn).len(),
                "has_reshared": user.clone().map(|u| u.has_reshared(&*conn, &post)).unwrap_or(false),
                "account": user,
                "date": &post.creation_date.timestamp(),
                "previous": query.and_then(|q| q.responding_to.map(|r| Comment::get(&*conn, r).expect("Error retrieving previous comment").to_json(&*conn))),
                "user_fqn": user.map(|u| u.get_fqn(&*conn)).unwrap_or(String::new())
            }))
        })
    })
}

#[get("/~/<blog>/<slug>", rank = 3, format = "application/activity+json")]
fn activity_details(blog: String, slug: String, conn: DbConn) -> ActivityStream<Article> {
    let blog = Blog::find_by_fqn(&*conn, blog).unwrap();
    let post = Post::find_by_slug(&*conn, slug, blog.id).unwrap();

    ActivityStream::new(post.into_activity(&*conn))
}

#[get("/~/<blog>/new", rank = 2)]
fn new_auth(blog: String) -> Flash<Redirect> {
    utils::requires_login("You need to be logged in order to write a new post", uri!(new: blog = blog))
}

#[get("/~/<blog>/new", rank = 1)]
fn new(blog: String, user: User, conn: DbConn) -> Template {
    let b = Blog::find_by_fqn(&*conn, blog.to_string()).unwrap();

    if !user.is_author_in(&*conn, b.clone()) {
        Template::render("errors/403", json!({
            "error_message": "You are not author in this blog."
        }))
    } else {
        Template::render("posts/new", json!({
            "account": user
        }))
    }
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

    if !user.is_author_in(&*conn, blog.clone()) {
        Redirect::to(uri!(super::blogs::details: name = blog_name))
    } else {
        if slug == "new" || Post::find_by_slug(&*conn, slug.clone(), blog.id).is_some() {
            Redirect::to(uri!(new: blog = blog_name))
        } else {
            let (content, mentions) = utils::md_to_html(form.content.to_string().as_ref());

            let post = Post::insert(&*conn, NewPost {
                blog_id: blog.id,
                slug: slug.to_string(),
                title: form.title.to_string(),
                content: SafeString::new(&content),
                published: true,
                license: form.license.to_string(),
                ap_url: "".to_string()
            });
            let post = post.update_ap_url(&*conn);
            PostAuthor::insert(&*conn, NewPostAuthor {
                post_id: post.id,
                author_id: user.id
            });

            for m in mentions.into_iter() {
                Mention::from_activity(&*conn, Mention::build_activity(&*conn, m), post.id, true);
            }

            let act = post.create_activity(&*conn);
            broadcast(&user, act, user.get_followers(&*conn));

            Redirect::to(uri!(details: blog = blog_name, slug = slug))
        }
    }
}
