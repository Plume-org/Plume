use rocket::response::{Flash, Redirect};
use rocket_i18n::I18n;

use crate::routes::errors::ErrorPage;
use crate::utils::requires_login;
use plume_common::activity_pub::broadcast;
use plume_models::{
    blogs::Blog, db_conn::DbConn, inbox::inbox, likes, posts::Post, timeline::*, users::User,
    Error, PlumeRocket, CONFIG,
};

#[post("/~/<blog>/<slug>/like")]
pub fn create(
    blog: String,
    slug: String,
    user: User,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Redirect, ErrorPage> {
    let b = Blog::find_by_fqn(&conn, &blog)?;
    let post = Post::find_by_slug(&conn, &slug, b.id)?;

    if !user.has_liked(&conn, &post)? {
        let like = likes::Like::insert(&conn, likes::NewLike::new(&post, &user))?;
        like.notify(&conn)?;

        Timeline::add_to_all_timelines(&conn, &post, Kind::Like(&user))?;

        let dest = User::one_by_instance(&conn)?;
        let act = like.to_activity(&conn)?;
        rockets
            .worker
            .execute(move || broadcast(&user, act, dest, CONFIG.proxy().cloned()));
    } else {
        let like = likes::Like::find_by_user_on_post(&conn, user.id, post.id)?;
        let delete_act = like.build_undo(&conn)?;
        inbox(
            &conn,
            serde_json::to_value(&delete_act).map_err(Error::from)?,
        )?;

        let dest = User::one_by_instance(&conn)?;
        rockets
            .worker
            .execute(move || broadcast(&user, delete_act, dest, CONFIG.proxy().cloned()));
    }

    Ok(Redirect::to(uri!(
        super::posts::details: blog = blog,
        slug = slug,
        responding_to = _
    )))
}

#[post("/~/<blog>/<slug>/like", rank = 2)]
pub fn create_auth(blog: String, slug: String, i18n: I18n) -> Flash<Redirect> {
    requires_login(
        &i18n!(i18n.catalog, "To like a post, you need to be logged in"),
        uri!(create: blog = blog, slug = slug),
    )
}
