use activitypub::object::Note;
use rocket::{
    request::LenientForm,
    response::Redirect
};
use rocket_i18n::I18n;
use validator::Validate;
use template_utils::Ructe;

use plume_common::{utils, activity_pub::{broadcast, ApRequest, 
    ActivityStream, inbox::Deletable}};
use plume_models::{
    blogs::Blog,
    comments::*,
    db_conn::DbConn,
    instance::Instance,
    mentions::Mention,
    posts::Post,
    safe_string::SafeString,
    tags::Tag,
    users::User
};
use Worker;

#[derive(Default, FromForm, Debug, Validate, Serialize)]
pub struct NewCommentForm {
    pub responding_to: Option<i32>,
    #[validate(length(min = "1", message = "Your comment can't be empty"))]
    pub content: String,
    pub warning: String,
}

#[post("/~/<blog_name>/<slug>/comment", data = "<form>")]
pub fn create(blog_name: String, slug: String, form: LenientForm<NewCommentForm>, user: User, conn: DbConn, worker: Worker, intl: I18n)
    -> Result<Redirect, Option<Ructe>> {
    let blog = Blog::find_by_fqn(&*conn, &blog_name).ok_or(None)?;
    let post = Post::find_by_slug(&*conn, &slug, blog.id).ok_or(None)?;
    form.validate()
        .map(|_| {
            let (html, mentions, _hashtags) = utils::md_to_html(form.content.as_ref(), &Instance::get_local(&conn).expect("comments::create: Error getting local instance").public_domain);
            let comm = Comment::insert(&*conn, NewComment {
                content: SafeString::new(html.as_ref()),
                in_response_to_id: form.responding_to,
                post_id: post.id,
                author_id: user.id,
                ap_url: None,
                sensitive: !form.warning.is_empty(),
                spoiler_text: form.warning.clone()
            }).update_ap_url(&*conn);
            let new_comment = comm.create_activity(&*conn);

            // save mentions
            for ment in mentions {
                Mention::from_activity(&*conn, &Mention::build_activity(&*conn, &ment), post.id, true, true);
            }

            // federate
            let dest = User::one_by_instance(&*conn);
            let user_clone = user.clone();
            worker.execute(move || broadcast(&user_clone, new_comment, dest));

            Redirect::to(uri!(super::posts::details: blog = blog_name, slug = slug, responding_to = _))
        })
        .map_err(|errors| {
            // TODO: de-duplicate this code
            let comments = Comment::list_by_post(&*conn, post.id);

            let previous = form.responding_to.map(|r| Comment::get(&*conn, r)
                .expect("comments::create: Error retrieving previous comment"));

            Some(render!(posts::details(
                &(&*conn, &intl.catalog, Some(user.clone())),
                post.clone(),
                blog,
                &*form,
                errors,
                Tag::for_post(&*conn, post.id),
                comments.into_iter().filter(|c| c.in_response_to_id.is_none()).collect::<Vec<Comment>>(),
                previous,
                post.count_likes(&*conn),
                post.count_reshares(&*conn),
                user.has_liked(&*conn, &post),
                user.has_reshared(&*conn, &post),
                user.is_following(&*conn, post.get_authors(&*conn)[0].id),
                post.get_authors(&*conn)[0].clone()
            )))
        })
}

#[post("/~/<blog>/<slug>/comment/<id>/delete")]
pub fn delete(blog: String, slug: String, id: i32, user: User, conn: DbConn, worker: Worker) -> Redirect {
    if let Some(comment) = Comment::get(&*conn, id) {
        if comment.author_id == user.id {
            let dest = User::one_by_instance(&*conn);
            let delete_activity = comment.delete(&*conn);
            worker.execute(move || broadcast(&user, delete_activity, dest));
        }
    }
    Redirect::to(uri!(super::posts::details: blog = blog, slug = slug, responding_to = _))
}

#[get("/~/<_blog>/<_slug>/comment/<id>")]
pub fn activity_pub(_blog: String, _slug: String, id: i32, _ap: ApRequest, conn: DbConn) -> Option<ActivityStream<Note>> {
    Comment::get(&*conn, id).map(|c| ActivityStream::new(c.to_activity(&*conn)))
}
