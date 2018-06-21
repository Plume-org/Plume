use rocket::{
    request::Form,
    response::{Redirect, Flash}
};
use rocket_contrib::Template;
use serde_json;

use activity_pub::{broadcast, inbox::Inbox, inbox::Notify};
use db_conn::DbConn;
use models::{
    blogs::Blog,
    comments::*,
    instance::Instance,
    posts::Post,
    users::User
};

use utils;

#[get("/~/<blog>/<slug>/comment")]
fn new(blog: String, slug: String, user: User, conn: DbConn) -> Template {
    may_fail!(Blog::find_by_fqn(&*conn, blog), "Couldn't find this blog", |blog| {
        may_fail!(Post::find_by_slug(&*conn, slug, blog.id), "Couldn't find this post", |post| {
            Template::render("comments/new", json!({
                "post": post,
                "account": user
            }))
        })
    })
}

#[get("/~/<blog>/<slug>/comment", rank=2)]
fn new_auth(blog: String, slug: String) -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to post a comment", uri!(new: blog = blog, slug = slug))
}

#[derive(FromForm)]
struct CommentQuery {
    responding_to: Option<i32>
}

#[derive(FromForm)]
struct NewCommentForm {
    pub content: String
}

// See: https://github.com/SergioBenitez/Rocket/pull/454
#[post("/~/<blog_name>/<slug>/comment", data = "<data>")]
fn create(blog_name: String, slug: String, data: Form<NewCommentForm>, user: User, conn: DbConn) -> Redirect {
    create_response(blog_name, slug, None, data, user, conn)
}

#[post("/~/<blog_name>/<slug>/comment?<query>", data = "<data>")]
fn create_response(blog_name: String, slug: String, query: Option<CommentQuery>, data: Form<NewCommentForm>, user: User, conn: DbConn) -> Redirect {
    let blog = Blog::find_by_fqn(&*conn, blog_name.clone()).unwrap();
    let post = Post::find_by_slug(&*conn, slug.clone(), blog.id).unwrap();
    let form = data.get();
<<<<<<< HEAD

    let (new_comment, id) = NewComment::build()
        .content(form.content.clone())
        .in_response_to_id(query.and_then(|q| q.responding_to))
        .post(post)
        .author(user.clone())
        .create(&*conn);

    // Comment::notify(&*conn, new_comment, user.clone().into_id());
    let instance = Instance::get_local(&*conn).unwrap();
    instance.received(&*conn, serde_json::to_value(new_comment.clone()).expect("JSON serialization error"));
    broadcast(&*conn, &user, new_comment, user.get_followers(&*conn));
=======
    let comment = Comment::insert(&*conn, NewComment {
        content: SafeString::new(&form.content.clone()),
        in_response_to_id: query.responding_to,
        post_id: post.id,
        author_id: user.id,
        ap_url: None, // TODO: set it
        sensitive: false,
        spoiler_text: "".to_string()
    });
    comment.notify(&*conn);

    broadcast(&*conn, &user, comment.create_activity(&*conn), user.get_followers(&*conn));
>>>>>>> dbdcbe71049e181c1c7649169c0153b3c9d81ad8

    Redirect::to(format!("/~/{}/{}/#comment-{}", blog_name, slug, id))
}
