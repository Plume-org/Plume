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
pub async fn create(
    blog: &str,
    slug: &str,
    user: User,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Redirect, ErrorPage> {
    let b = Blog::find_by_fqn(&mut conn, blog).await?;
    let post = Post::find_by_slug(&mut conn, slug, b.id)?;

    if !user.has_reshared(&mut conn, &post)? {
        let reshare = Reshare::insert(&mut conn, NewReshare::new(&post, &user))?;
        reshare.notify(&mut conn)?;

        Timeline::add_to_all_timelines(&mut conn, &post, &Kind::Reshare(user.clone())).await?;

        let dest = User::one_by_instance(&mut conn)?;
        let act = reshare.to_activity(&mut conn)?;
        rockets
            .worker
            .execute(move || broadcast(&user, act, dest, CONFIG.proxy().cloned()));
    } else {
        let reshare = Reshare::find_by_user_on_post(&mut conn, user.id, post.id)?;
        let delete_act = reshare.build_undo(&mut conn)?;
        inbox(
            &mut conn,
            serde_json::to_value(&delete_act).map_err(Error::from)?,
        ).await?;

        let dest = User::one_by_instance(&mut conn)?;
        rockets
            .worker
            .execute(move || broadcast(&user, delete_act, dest, CONFIG.proxy().cloned()));
    }

    Ok(Redirect::to(uri!(
        super::posts::details(blog = blog,
        slug = slug,
        responding_to = _
    ))))
}

#[post("/~/<blog>/<slug>/reshare", rank = 1)]
pub fn create_auth(blog: String, slug: &str, i18n: I18n) -> Flash<Redirect> {
    requires_login(
        &i18n!(i18n.catalog, "To reshare a post, you need to be logged in"),
        uri!(create(blog = blog, slug = slug)),
    )
}
