use rocket::{
    request::Form,
    response::{Redirect, Flash}
};
use rocket_contrib::Template;

use activity_pub::broadcast;
use db_conn::DbConn;
use models::{
    comments::*,
    posts::Post,
    users::User
};

use utils;

#[get("/~/<_blog>/<slug>/comment")]
fn new(_blog: String, slug: String, user: User, conn: DbConn) -> Template {
    let post = Post::find_by_slug(&*conn, slug).unwrap();
    Template::render("comments/new", json!({
        "post": post,
        "account": user
    }))
}

#[get("/~/<blog>/<slug>/comment", rank=2)]
fn new_auth(blog: String, slug: String) -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to post a comment", &format!("~/{}/{}/comment", blog, slug))
}

#[derive(FromForm)]
struct CommentQuery {
    responding_to: Option<i32>
}

#[derive(FromForm)]
struct NewCommentForm {
    pub content: String
}

#[post("/~/<blog>/<slug>/comment?<query>", data = "<data>")]
fn create(blog: String, slug: String, query: CommentQuery, data: Form<NewCommentForm>, user: User, conn: DbConn) -> Redirect {
    let post = Post::find_by_slug(&*conn, slug.clone()).unwrap();
    let form = data.get();
    let comment = Comment::insert(&*conn, NewComment {
        content: form.content.clone(),
        in_response_to_id: query.responding_to,
        post_id: post.id,
        author_id: user.id,
        ap_url: None,
        sensitive: false,
        spoiler_text: "".to_string()
    });

    broadcast(&*conn, &user, comment.create_activity(&*conn), user.get_followers(&*conn));

    Redirect::to(format!("/~/{}/{}/#comment-{}", blog, slug, comment.id).as_ref())
}
