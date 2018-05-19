use rocket::response::Redirect;

use activity_pub::broadcast;
use db_conn::DbConn;
use models::{
    posts::Post,
    reshares::*,
    users::User
};

#[get("/~/<blog>/<slug>/reshare")]
fn create(blog: String, slug: String, user: User, conn: DbConn) -> Redirect {
    let post = Post::find_by_slug(&*conn, slug.clone()).unwrap();

    if !user.has_reshared(&*conn, &post) {
        let reshare = Reshare::insert(&*conn, NewReshare {
            post_id: post.id,
            user_id: user.id,
            ap_url: "".to_string()
        });
        reshare.update_ap_url(&*conn);

        broadcast(&*conn, &user, reshare.into_activity(&*conn), user.get_followers(&*conn));
    } else {
        let reshare = Reshare::find_by_user_on_post(&*conn, &user, &post).unwrap();
        let delete_act = reshare.delete(&*conn);
        broadcast(&*conn, &user, delete_act, user.get_followers(&*conn));
    }

    Redirect::to(format!("/~/{}/{}/", blog, slug).as_ref())
}
