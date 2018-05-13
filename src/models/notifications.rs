use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods};

use schema::notifications;

#[derive(Queryable, Identifiable)]
pub struct Notification {
    pub id: i32,
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
    pub user_id: i32
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
}
