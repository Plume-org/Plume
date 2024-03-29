use rocket::response::{Flash, Redirect};
use rocket_i18n::I18n;

use crate::routes::errors::ErrorPage;
use crate::utils::requires_login;
use plume_common::activity_pub::broadcast;
use plume_models::{
    blogs::Blog, db_conn::DbConn, inbox::inbox, posts::Post, reshares::*, timeline::*, users::User,
    Error, PlumeRocket, CONFIG,
};

#[post("/~/<blog>/<slug>/reshare")]
pub fn create(
    blog: String,
    slug: String,
    user: User,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Redirect, ErrorPage> {
    let b = Blog::find_by_fqn(&conn, &blog)?;
    let post = Post::find_by_slug(&conn, &slug, b.id)?;

    if !user.has_reshared(&conn, &post)? {
        let reshare = Reshare::insert(&conn, NewReshare::new(&post, &user))?;
        reshare.notify(&conn)?;

        Timeline::add_to_all_timelines(&conn, &post, Kind::Reshare(&user))?;

        let dest = User::one_by_instance(&conn)?;
        let act = reshare.to_activity(&conn)?;
        rockets
            .worker
            .execute(move || broadcast(&user, act, dest, CONFIG.proxy().cloned()));
    } else {
        let reshare = Reshare::find_by_user_on_post(&conn, user.id, post.id)?;
        let delete_act = reshare.build_undo(&conn)?;
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

#[post("/~/<blog>/<slug>/reshare", rank = 1)]
pub fn create_auth(blog: String, slug: String, i18n: I18n) -> Flash<Redirect> {
    requires_login(
        &i18n!(i18n.catalog, "To reshare a post, you need to be logged in"),
        uri!(create: blog = blog, slug = slug),
    )
}
