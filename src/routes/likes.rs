use rocket::{State, response::{Redirect, Flash}};
use workerpool::{Pool, thunk::*};

use plume_common::activity_pub::{broadcast, inbox::{Notify, Deletable}};
use plume_common::utils;
use plume_models::{
    blogs::Blog,
    db_conn::DbConn,
    likes,
    posts::Post,
    users::User
};

#[post("/~/<blog>/<slug>/like")]
pub fn create(blog: String, slug: String, user: User, conn: DbConn, worker: State<Pool<ThunkWorker<()>>>) -> Option<Redirect> {
    let b = Blog::find_by_fqn(&*conn, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, b.id)?;

    if !user.has_liked(&*conn, &post) {
        let like = likes::Like::insert(&*conn, likes::NewLike {
            post_id: post.id,
            user_id: user.id,
            ap_url: "".to_string()
        });
        like.update_ap_url(&*conn);
        like.notify(&*conn);

        let dest = User::one_by_instance(&*conn);
        let act = like.to_activity(&*conn);
        worker.execute(Thunk::of(move || broadcast(&user, act, dest)));
    } else {
        let like = likes::Like::find_by_user_on_post(&*conn, user.id, post.id).expect("likes::create: like exist but not found error");
        let delete_act = like.delete(&*conn);
        let dest = User::one_by_instance(&*conn);
        worker.execute(Thunk::of(move || broadcast(&user, delete_act, dest)));
    }

    Some(Redirect::to(uri!(super::posts::details: blog = blog, slug = slug)))
}

#[post("/~/<blog>/<slug>/like", rank = 2)]
pub fn create_auth(blog: String, slug: String) -> Flash<Redirect>{
    utils::requires_login(
        "You need to be logged in order to like a post",
        uri!(create: blog = blog, slug = slug)
    )
}
