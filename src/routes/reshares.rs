use rocket::response::{Flash, Redirect};
use rocket_i18n::I18n;

use plume_common::activity_pub::{
    broadcast,
    inbox::{Deletable, Notify},
};
use plume_common::utils;
use plume_models::{blogs::Blog, db_conn::DbConn, posts::Post, reshares::*, users::User};
use routes::errors::ErrorPage;
use Worker;

#[post("/~/<blog>/<slug>/reshare")]
pub fn create(
    blog: String,
    slug: String,
    user: User,
    conn: DbConn,
    worker: Worker,
) -> Result<Redirect, ErrorPage> {
    let b = Blog::find_by_fqn(&*conn, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, b.id)?;

    if !user.has_reshared(&*conn, &post)? {
        let reshare = Reshare::insert(&*conn, NewReshare::new(&post, &user))?;
        reshare.notify(&*conn)?;

        let dest = User::one_by_instance(&*conn)?;
        let act = reshare.to_activity(&*conn)?;
        worker.execute(move || broadcast(&user, act, dest));
    } else {
        let reshare = Reshare::find_by_user_on_post(&*conn, user.id, post.id)?;
        let delete_act = reshare.delete(&*conn)?;
        let dest = User::one_by_instance(&*conn)?;
        worker.execute(move || broadcast(&user, delete_act, dest));
    }

    Ok(Redirect::to(
        uri!(super::posts::details: blog = blog, slug = slug, responding_to = _),
    ))
}

#[post("/~/<blog>/<slug>/reshare", rank = 1)]
pub fn create_auth(blog: String, slug: String, i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(
            i18n.catalog,
            "To reshare a post, you need to be logged in"
        ),
        uri!(create: blog = blog, slug = slug),
    )
}
