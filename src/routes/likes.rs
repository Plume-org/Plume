use rocket::response::Redirect;

use activity_pub::activity::Like;
use activity_pub::outbox::broadcast;
use db_conn::DbConn;
use models::likes;
use models::posts::Post;
use models::users::User;

#[get("/~/<blog>/<slug>/like")]
fn create(blog: String, slug: String, user: User, conn: DbConn) -> Redirect {
    let post = Post::find_by_slug(&*conn, slug.clone()).unwrap();
    likes::Like::insert(&*conn, likes::NewLike {
        post_id: post.id,
        user_id: user.id
    });
    let act = Like::new(&user, &post, &*conn);
    broadcast(&*conn, &user, act, user.get_followers(&*conn));

    Redirect::to(format!("/~/{}/{}/", blog, slug).as_ref())
}
