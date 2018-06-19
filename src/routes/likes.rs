use rocket::response::{Redirect, Flash};

use activity_pub::{broadcast, IntoId, inbox::Notify};
use db_conn::DbConn;
use models::{
    blogs::Blog,
    likes,
    posts::Post,
    users::User
};

use utils;

#[get("/~/<blog>/<slug>/like")]
fn create(blog: String, slug: String, user: User, conn: DbConn) -> Redirect {
    let b = Blog::find_by_fqn(&*conn, blog.clone()).unwrap();
    let post = Post::find_by_slug(&*conn, slug.clone(), b.id).unwrap();

    if !user.has_liked(&*conn, &post) {
        let like = likes::Like::insert(&*conn, likes::NewLike {
            post_id: post.id,
            user_id: user.id,
            ap_url: "".to_string()
        });
        like.update_ap_url(&*conn);

        likes::Like::notify(&*conn, like.into_activity(&*conn), user.clone().into_id());
        broadcast(&*conn, &user, like.into_activity(&*conn), user.get_followers(&*conn));
    } else {
        let like = likes::Like::find_by_user_on_post(&*conn, user.id, post.id).unwrap();
        let delete_act = like.delete(&*conn);
        broadcast(&*conn, &user, delete_act, user.get_followers(&*conn));
    }

    Redirect::to(uri!(super::posts::details: blog = blog, slug = slug))
}

#[get("/~/<blog>/<slug>/like", rank = 2)]
fn create_auth(blog: String, slug: String) -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to like a post", uri!(create: blog = blog, slug = slug))
}
