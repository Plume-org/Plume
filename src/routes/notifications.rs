use rocket_contrib::Template;

use db_conn::DbConn;
use models::notifications::Notification;
use models::users::User;

#[get("/notifications")]
fn notifications(conn: DbConn, user: User) -> Template {
    Template::render("notifications/index", json!({
        "account": user,
        "notifications": Notification::find_for_user(&*conn, &user)
    }))
}
