use chrono::NaiveDateTime;
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods};

use models::users::User;
use schema::notifications;

#[derive(Queryable, Identifiable, Serialize)]
pub struct Notification {
    pub id: i32,
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
    pub user_id: i32,
    pub creation_date: NaiveDateTime
}

#[derive(Insertable)]
#[table_name = "notifications"]
pub struct NewNotification {
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
    pub user_id: i32
}

impl Notification {
    pub fn insert(conn: &PgConnection, new: NewNotification) -> Notification {
        diesel::insert_into(notifications::table)
            .values(new)
            .get_result(conn)
            .expect("Couldn't save notification")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Notification> {
        notifications::table.filter(notifications::id.eq(id))
            .limit(1)
            .load::<Notification>(conn)
            .expect("Couldn't load notification by ID")
            .into_iter().nth(0)
    }

    pub fn find_for_user(conn: &PgConnection, user: &User) -> Vec<Notification> {
        notifications::table.filter(notifications::user_id.eq(user.id))
            .order_by(notifications::creation_date.desc())
            .load::<Notification>(conn)
            .expect("Couldn't load user notifications")
    }
}
