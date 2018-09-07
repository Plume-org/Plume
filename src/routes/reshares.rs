use rocket::{State, response::{Redirect, Flash}};
use workerpool::{Pool, thunk::*};

use plume_common::activity_pub::{broadcast, inbox::{Deletable, Notify}};
use plume_common::utils;
use plume_models::{
    blogs::Blog,
    db_conn::DbConn,
    posts::Post,
    reshares::*,
    users::User
};

#[post("/~/<blog>/<slug>/reshare")]
fn create(blog: String, slug: String, user: User, conn: DbConn, worker: State<Pool<ThunkWorker<()>>>) -> Redirect {
    let b = Blog::find_by_fqn(&*conn, blog.clone()).unwrap();
    let post = Post::find_by_slug(&*conn, slug.clone(), b.id).unwrap();

    if !user.has_reshared(&*conn, &post) {
        let reshare = Reshare::insert(&*conn, NewReshare {
            post_id: post.id,
            user_id: user.id,
            ap_url: "".to_string()
        });
        reshare.update_ap_url(&*conn);
        reshare.notify(&*conn);

        let followers = user.get_followers(&*conn);
        let act = reshare.into_activity(&*conn);
        worker.execute(Thunk::of(move || broadcast(&user, act, followers)));
    } else {
        let reshare = Reshare::find_by_user_on_post(&*conn, user.id, post.id).unwrap();
        let delete_act = reshare.delete(&*conn);
        let followers = user.get_followers(&*conn);
        worker.execute(Thunk::of(move || broadcast(&user, delete_act, followers)));
    }

    Redirect::to(uri!(super::posts::details: blog = blog, slug = slug))
}

#[post("/~/<blog>/<slug>/reshare", rank=1)]
fn create_auth(blog: String, slug: String) -> Flash<Redirect> {
    utils::requires_login(
        "You need to be logged in order to reshare a post",
        uri!(create: blog = blog, slug = slug).into()
    )
}
