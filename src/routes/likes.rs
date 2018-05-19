use rocket::response::Redirect;

use activity_pub::broadcast;
use db_conn::DbConn;
use models::{
    likes,
    posts::Post,
    users::User
};

#[get("/~/<blog>/<slug>/like")]
fn create(blog: String, slug: String, user: User, conn: DbConn) -> Redirect {
    let post = Post::find_by_slug(&*conn, slug.clone()).unwrap();

    if !user.has_liked(&*conn, &post) {
        let like = likes::Like::insert(&*conn, likes::NewLike {
                post_id: post.id,
                user_id: user.id,
                ap_url: "".to_string()
        });
        like.update_ap_url(&*conn);

        broadcast(&*conn, &user, like.into_activity(&*conn), user.get_followers(&*conn));
    } else {
        let like = likes::Like::find_by_user_on_post(&*conn, &user, &post).unwrap();
        let delete_act = like.delete(&*conn);
        broadcast(&*conn, &user, delete_act, user.get_followers(&*conn));
    }

    Redirect::to(format!("/~/{}/{}/", blog, slug).as_ref())
}
