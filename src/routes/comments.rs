use activitystreams_types::activity::Create;
use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;

use activity_pub::broadcast;
use db_conn::DbConn;
use models::comments::*;
use models::posts::Post;
use models::users::User;

#[get("/~/<_blog>/<slug>/comment")]
fn new(_blog: String, slug: String, user: User, conn: DbConn) -> Template {
    let post = Post::find_by_slug(&*conn, slug).unwrap();
    Template::render("comments/new", json!({
        "post": post,
        "account": user
    }))
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
    // TODO: let act = Create::new(&user, &comment, &*conn);
    // broadcast(&*conn, &user, act, user.get_followers(&*conn));

    Redirect::to(format!("/~/{}/{}/#comment-{}", blog, slug, comment.id).as_ref())
}
