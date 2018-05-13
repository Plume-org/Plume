use rocket::response::Redirect;

use activity_pub::activity::{Like, Undo};
use activity_pub::outbox::broadcast;
use db_conn::DbConn;
use models::likes;
use models::posts::Post;
use models::users::User;

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
        let act = Like::new(&user, &post, &*conn);
        broadcast(&*conn, &user, act, user.get_followers(&*conn));
    } else {
        let like = likes::Like::for_user_on_post(&*conn, &user, &post).unwrap();
        like.delete(&*conn);
        broadcast(&*conn, &user, Undo::new(&user, &like, &*conn), user.get_followers(&*conn));
    }

    Redirect::to(format!("/~/{}/{}/", blog, slug).as_ref())
}
