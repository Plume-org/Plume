use rocket::request::Form;
use rocket::response::Redirect;
use rocket_contrib::Template;

use db_conn::DbConn;
use models::comments::*;
use models::posts::Post;
use models::users::User;

#[get("/~/<_blog>/<slug>/comment")]
fn new(_blog: String, slug: String, _user: User, conn: DbConn) -> Template {
    let post = Post::find_by_slug(&*conn, slug).unwrap();
    Template::render("comments/new", json!({
        "post": post
    }))
}

#[derive(FromForm)]
struct NewCommentForm {
    pub content: String,
    pub respond_to: Option<i32>
}

#[post("/~/<blog>/<slug>/comment", data = "<data>")]
fn create(blog: String, slug: String, data: Form<NewCommentForm>, user: User, conn: DbConn) -> Redirect {
    let post = Post::find_by_slug(&*conn, slug.clone()).unwrap();
    let form = data.get();
    let comment = Comment::insert(&*conn, NewComment {
        content: form.content.clone(),
        in_response_to_id: form.respond_to,
        post_id: post.id,
        author_id: user.id,
        ap_url: None,
        sensitive: false,
        spoiler_text: "".to_string()
    });
    Redirect::to(format!("/~/{}/{}/#comment-{}", blog, slug, comment.id).as_ref())
}
