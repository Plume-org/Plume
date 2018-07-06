use rocket::{
    request::LenientForm,
    response::Redirect
};
use rocket_contrib::Template;
use serde_json;
use validator::Validate;

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

#[derive(FromForm, Debug, Validate)]
struct NewCommentForm {
    pub responding_to: Option<i32>,
    #[validate(length(min = "1"))]
    pub content: String
}

#[post("/~/<blog_name>/<slug>/comment", data = "<data>")]
fn create(blog_name: String, slug: String, data: LenientForm<NewCommentForm>, user: User, conn: DbConn) -> Result<Redirect, Template> {
    let blog = Blog::find_by_fqn(&*conn, blog_name.clone()).unwrap();
    let post = Post::find_by_slug(&*conn, slug.clone(), blog.id).unwrap();
    let form = data.get();
    form.validate()
        .map(|_| {
            let (new_comment, id) = NewComment::build()
                .content(form.content.clone())
                .in_response_to_id(form.responding_to.clone())
                .post(post.clone())
                .author(user.clone())
                .create(&*conn);

            let instance = Instance::get_local(&*conn).unwrap();
            instance.received(&*conn, serde_json::to_value(new_comment.clone()).expect("JSON serialization error"))
                .expect("We are not compatible with ourselve: local broadcast failed (new comment)");
            broadcast(&user, new_comment, user.get_followers(&*conn));

            Redirect::to(format!("/~/{}/{}/#comment-{}", blog_name, slug, id))
        })
        .map_err(|errors| {
            // TODO: de-duplicate this code
            let comments = Comment::list_by_post(&*conn, post.id);

            Template::render("posts/details", json!({
                "author": post.get_authors(&*conn)[0].to_json(&*conn),
                "post": post,
                "blog": blog,
                "comments": comments.into_iter().map(|c| c.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
                "n_likes": post.get_likes(&*conn).len(),
                "has_liked": user.has_liked(&*conn, &post),
                "n_reshares": post.get_reshares(&*conn).len(),
                "has_reshared": user.has_reshared(&*conn, &post),
                "account": user,
                "date": &post.creation_date.timestamp(),
                "previous": form.responding_to.map(|r| Comment::get(&*conn, r).expect("Error retrieving previous comment").to_json(&*conn)),
                "user_fqn": user.get_fqn(&*conn),
                "errors": errors
            }))
        })    
}
