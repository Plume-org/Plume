use activitypub::object::Note;
use rocket::{
    request::LenientForm,
    response::{Flash, Redirect},
};
use template_utils::Ructe;
use validator::Validate;

use std::time::Duration;

use plume_common::{
    activity_pub::{broadcast, ActivityStream, ApRequest},
    utils,
};
use plume_models::{
    blogs::Blog, comments::*, inbox::inbox, instance::Instance, medias::Media, mentions::Mention,
    posts::Post, safe_string::SafeString, tags::Tag, users::User, Error, PlumeRocket,
};
use routes::errors::ErrorPage;
use template_utils::IntoContext;

#[derive(Default, FromForm, Debug, Validate)]
pub struct NewCommentForm {
    pub responding_to: Option<i32>,
    #[validate(length(min = "1", message = "Your comment can't be empty"))]
    pub content: String,
    pub warning: String,
}

#[post("/~/<blog_name>/<slug>/comment", data = "<form>")]
pub fn create(
    blog_name: String,
    slug: String,
    form: LenientForm<NewCommentForm>,
    user: User,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, Ructe> {
    let conn = &*rockets.conn;
    let blog = Blog::find_by_fqn(&rockets, &blog_name).expect("comments::create: blog error");
    let post = Post::find_by_slug(&*conn, &slug, blog.id).expect("comments::create: post error");
    form.validate()
        .map(|_| {
            let (html, mentions, _hashtags) = utils::md_to_html(
                form.content.as_ref(),
                &Instance::get_local(&conn)
                    .expect("comments::create: local instance error")
                    .public_domain,
                true,
                Some(Media::get_media_processor(&conn, vec![&user])),
            );
            let comm = Comment::insert(
                &*conn,
                NewComment {
                    content: SafeString::new(html.as_ref()),
                    in_response_to_id: form.responding_to,
                    post_id: post.id,
                    author_id: user.id,
                    ap_url: None,
                    sensitive: !form.warning.is_empty(),
                    spoiler_text: form.warning.clone(),
                    public_visibility: true,
                },
            )
            .expect("comments::create: insert error");
            comm.notify(&*conn).expect("comments::create: notify error");
            let new_comment = comm
                .create_activity(&rockets)
                .expect("comments::create: activity error");

            // save mentions
            for ment in mentions {
                Mention::from_activity(
                    &*conn,
                    &Mention::build_activity(&rockets, &ment)
                        .expect("comments::create: build mention error"),
                    comm.id,
                    false,
                    true,
                )
                .expect("comments::create: mention save error");
            }

            // federate
            let dest = User::one_by_instance(&*conn).expect("comments::create: dest error");
            let user_clone = user.clone();
            rockets
                .worker
                .execute(move || broadcast(&user_clone, new_comment, dest));

            Flash::success(
                Redirect::to(
                    uri!(super::posts::details: blog = blog_name, slug = slug, responding_to = _),
                ),
                i18n!(&rockets.intl.catalog, "Your comment have been posted."),
            )
        })
        .map_err(|errors| {
            // TODO: de-duplicate this code
            let comments = CommentTree::from_post(&*conn, &post, Some(&user))
                .expect("comments::create: comments error");

            let previous = form
                .responding_to
                .and_then(|r| Comment::get(&*conn, r).ok());

            render!(posts::details(
                &rockets.to_context(),
                post.clone(),
                blog,
                &*form,
                errors,
                Tag::for_post(&*conn, post.id).expect("comments::create: tags error"),
                comments,
                previous,
                post.count_likes(&*conn)
                    .expect("comments::create: count likes error"),
                post.count_reshares(&*conn)
                    .expect("comments::create: count reshares error"),
                user.has_liked(&*conn, &post)
                    .expect("comments::create: liked error"),
                user.has_reshared(&*conn, &post)
                    .expect("comments::create: reshared error"),
                user.is_following(
                    &*conn,
                    post.get_authors(&*conn)
                        .expect("comments::create: authors error")[0]
                        .id
                )
                .expect("comments::create: following error"),
                post.get_authors(&*conn)
                    .expect("comments::create: authors error")[0]
                    .clone()
            ))
        })
}

#[post("/~/<blog>/<slug>/comment/<id>/delete")]
pub fn delete(
    blog: String,
    slug: String,
    id: i32,
    user: User,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, ErrorPage> {
    if let Ok(comment) = Comment::get(&*rockets.conn, id) {
        if comment.author_id == user.id {
            let dest = User::one_by_instance(&*rockets.conn)?;
            let delete_activity = comment.build_delete(&*rockets.conn)?;
            inbox(
                &rockets,
                serde_json::to_value(&delete_activity).map_err(Error::from)?,
            )?;

            let user_c = user.clone();
            rockets
                .worker
                .execute(move || broadcast(&user_c, delete_activity, dest));
            let conn = rockets.conn;
            rockets
                .worker
                .execute_after(Duration::from_secs(10 * 60), move || {
                    user.rotate_keypair(&conn)
                        .expect("Failed to rotate keypair");
                });
        }
    }
    Ok(Flash::success(
        Redirect::to(uri!(super::posts::details: blog = blog, slug = slug, responding_to = _)),
        i18n!(&rockets.intl.catalog, "Your comment have been deleted."),
    ))
}

#[get("/~/<_blog>/<_slug>/comment/<id>")]
pub fn activity_pub(
    _blog: String,
    _slug: String,
    id: i32,
    _ap: ApRequest,
    rockets: PlumeRocket,
) -> Option<ActivityStream<Note>> {
    Comment::get(&*rockets.conn, id)
        .and_then(|c| c.to_activity(&rockets))
        .ok()
        .map(ActivityStream::new)
}
