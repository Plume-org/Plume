use activitypub::object::Note;
use rocket::{
    State,
    request::LenientForm,
    response::Redirect
};
use rocket_contrib::Template;
use serde_json;
use validator::Validate;
use workerpool::{Pool, thunk::*};

use plume_common::{utils, activity_pub::{broadcast, ApRequest, ActivityStream}};
use plume_models::{
    blogs::Blog,
    comments::*,
    db_conn::DbConn,
    mentions::Mention,
    posts::Post,
    safe_string::SafeString,
    users::User
};

#[derive(FromForm, Debug, Validate, Serialize)]
struct NewCommentForm {
    pub responding_to: Option<i32>,
    #[validate(length(min = "1", message = "Your comment can't be empty"))]
    pub content: String,
    pub warning: String,
}

#[post("/~/<blog_name>/<slug>/comment", data = "<data>")]
fn create(blog_name: String, slug: String, data: LenientForm<NewCommentForm>, user: User, conn: DbConn, worker: State<Pool<ThunkWorker<()>>>)
    -> Result<Redirect, Option<Template>> {
    let blog = Blog::find_by_fqn(&*conn, blog_name.clone()).ok_or(None)?;
    let post = Post::find_by_slug(&*conn, slug.clone(), blog.id).ok_or(None)?;
    let form = data.get();
    form.validate()
        .map(|_| {
            let (html, mentions, _hashtags) = utils::md_to_html(form.content.as_ref());
            let comm = Comment::insert(&*conn, NewComment {
                content: SafeString::new(html.as_ref()),
                in_response_to_id: form.responding_to.clone(),
                post_id: post.id,
                author_id: user.id,
                ap_url: None,
                sensitive: form.warning.len() > 0,
                spoiler_text: form.warning.clone()
            }).update_ap_url(&*conn);
            let new_comment = comm.create_activity(&*conn);

            // save mentions
            for ment in mentions {
                Mention::from_activity(&*conn, Mention::build_activity(&*conn, ment), post.id, true, true);
            }

            // federate
            let dest = User::one_by_instance(&*conn);
            let user_clone = user.clone();
            worker.execute(Thunk::of(move || broadcast(&user_clone, new_comment, dest)));

            Redirect::to(uri!(super::posts::details: blog = blog_name, slug = slug))
        })
        .map_err(|errors| {
            // TODO: de-duplicate this code
            let comments = Comment::list_by_post(&*conn, post.id);
            let comms = comments.clone();

            Some(Template::render("posts/details", json!({
                "author": post.get_authors(&*conn)[0].to_json(&*conn),
                "post": post,
                "blog": blog,
                "comments": &comments.into_iter().map(|c| c.to_json(&*conn, &comms)).collect::<Vec<serde_json::Value>>(),
                "n_likes": post.get_likes(&*conn).len(),
                "has_liked": user.has_liked(&*conn, &post),
                "n_reshares": post.get_reshares(&*conn).len(),
                "has_reshared": user.has_reshared(&*conn, &post),
                "account": user.to_json(&*conn),
                "date": &post.creation_date.timestamp(),
                "previous": form.responding_to.and_then(|r| Comment::get(&*conn, r)).map(|r| r.to_json(&*conn, &vec![])),
                "user_fqn": user.get_fqn(&*conn),
                "comment_form": form,
                "comment_errors": errors,
            })))
        })
}

#[get("/~/<_blog>/<_slug>/comment/<id>")]
fn activity_pub(_blog: String, _slug: String, id: i32, _ap: ApRequest, conn: DbConn) -> Option<ActivityStream<Note>> {
    Comment::get(&*conn, id).map(|c| ActivityStream::new(c.into_activity(&*conn)))
}
