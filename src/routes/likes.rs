use rocket::response::{Redirect, Flash};
use rocket_i18n::I18n;

use plume_common::activity_pub::{broadcast, inbox::{Notify, Deletable}};
use plume_common::utils;
use plume_models::{
    blogs::Blog,
    db_conn::DbConn,
    likes,
    posts::Post,
    users::User
};
use Worker;
use routes::errors::ErrorPage;

#[post("/~/<blog>/<slug>/like")]
pub fn create(blog: String, slug: String, user: User, conn: DbConn, worker: Worker) -> Result<Redirect, ErrorPage> {
    let b = Blog::find_by_fqn(&*conn, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, b.id)?;

    if !user.has_liked(&*conn, &post)? {
        let like = likes::Like::insert(&*conn, likes::NewLike::new(&post ,&user))?;
        like.notify(&*conn)?;

        let dest = User::one_by_instance(&*conn)?;
        let act = like.to_activity(&*conn)?;
        worker.execute(move || broadcast(&user, act, dest));
    } else {
        let like = likes::Like::find_by_user_on_post(&*conn, user.id, post.id)?;
        let delete_act = like.delete(&*conn)?;
        let dest = User::one_by_instance(&*conn)?;
        worker.execute(move || broadcast(&user, delete_act, dest));
    }

    Ok(Redirect::to(uri!(super::posts::details: blog = blog, slug = slug, responding_to = _)))
}

#[post("/~/<blog>/<slug>/like", rank = 2)]
pub fn create_auth(blog: String, slug: String, i18n: I18n) -> Flash<Redirect>{
    utils::requires_login(
        &i18n!(i18n.catalog, "You need to be logged in to like a post"),
        uri!(create: blog = blog, slug = slug)
    )
}
