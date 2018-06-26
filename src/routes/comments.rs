use rocket::{
    request::LenientForm,
    response::Redirect
};
use serde_json;

use plume_common::activity_pub::broadcast;
use plume_models::{
    blogs::Blog,
    comments::*,
    db_conn::DbConn,
    instance::Instance,
    posts::Post,
    users::User
};
use inbox::Inbox;

#[derive(FromForm)]
pub struct CommentQuery {
    pub responding_to: Option<i32>
}

#[derive(FromForm)]
struct NewCommentForm {
    pub content: String
}

// See: https://github.com/SergioBenitez/Rocket/pull/454
#[post("/~/<blog_name>/<slug>/comment", data = "<data>")]
fn create(blog_name: String, slug: String, data: LenientForm<NewCommentForm>, user: User, conn: DbConn) -> Redirect {
    create_response(blog_name, slug, None, data, user, conn)
}

#[post("/~/<blog_name>/<slug>/comment?<query>", data = "<data>")]
fn create_response(blog_name: String, slug: String, query: Option<CommentQuery>, data: LenientForm<NewCommentForm>, user: User, conn: DbConn) -> Redirect {
    let blog = Blog::find_by_fqn(&*conn, blog_name.clone()).unwrap();
    let post = Post::find_by_slug(&*conn, slug.clone(), blog.id).unwrap();
    let form = data.get();

    let (new_comment, id) = NewComment::build()
        .content(form.content.clone())
        .in_response_to_id(query.and_then(|q| q.responding_to))
        .post(post)
        .author(user.clone())
        .create(&*conn);

    let instance = Instance::get_local(&*conn).unwrap();
    instance.received(&*conn, serde_json::to_value(new_comment.clone()).expect("JSON serialization error"))
        .expect("We are not compatible with ourselve: local broadcast failed (new comment)");
    broadcast(&user, new_comment, user.get_followers(&*conn));

    Redirect::to(format!("/~/{}/{}/#comment-{}", blog_name, slug, id))
}
