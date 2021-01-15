use rocket::response::{Flash, Redirect};
use rocket_i18n::I18n;

use crate::routes::errors::ErrorPage;
use plume_common::activity_pub::broadcast;
use plume_common::utils;
use plume_models::{
    blogs::Blog, inbox::inbox, posts::Post, reshares::*, timeline::*, users::User, Error,
    PlumeRocket, CONFIG,
};

#[post("/~/<blog>/<slug>/reshare")]
pub fn create(
    blog: String,
    slug: String,
    user: User,
    rockets: PlumeRocket,
) -> Result<Redirect, ErrorPage> {
    let conn = &*rockets.conn;
    let b = Blog::find_by_fqn(&rockets, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, b.id)?;

    if !user.has_reshared(&*conn, &post)? {
        let reshare = Reshare::insert(&*conn, NewReshare::new(&post, &user))?;
        reshare.notify(&*conn)?;

        Timeline::add_to_all_timelines(&rockets, &post, Kind::Reshare(&user))?;

        let dest = User::one_by_instance(&*conn)?;
        let act = reshare.to_activity(&*conn)?;
        rockets
            .worker
            .execute(move || broadcast(&user, act, dest, CONFIG.proxy().cloned()));
    } else {
        let reshare = Reshare::find_by_user_on_post(&*conn, user.id, post.id)?;
        let delete_act = reshare.build_undo(&*conn)?;
        inbox(
            &rockets,
            serde_json::to_value(&delete_act).map_err(Error::from)?,
        )?;

        let dest = User::one_by_instance(&*conn)?;
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
    utils::requires_login(
        &i18n!(i18n.catalog, "To reshare a post, you need to be logged in"),
        uri!(create: blog = blog, slug = slug),
    )
}
