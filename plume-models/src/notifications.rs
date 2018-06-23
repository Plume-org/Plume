use chrono::NaiveDateTime;
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods};

use users::User;
use schema::notifications;

#[derive(Queryable, Identifiable, Serialize)]
pub struct Notification {
    pub id: i32,
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
    pub user_id: i32,
    pub creation_date: NaiveDateTime,
    pub data: Option<String>
}

#[derive(Insertable)]
#[table_name = "notifications"]
pub struct NewNotification {
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
    pub user_id: i32,
    pub data: Option<String>
}

impl Notification {
    insert!(notifications, NewNotification);
    get!(notifications);

    pub fn find_for_user(conn: &PgConnection, user: &User) -> Vec<Notification> {
        notifications::table.filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .load::<Notification>(conn)
            .expect("Couldn't load user notifications")
    }
}
