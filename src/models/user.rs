use diesel;
use diesel::{ QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection };
use schema::users;

#[derive(Queryable, Identifiable)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub is_admin: bool,
    pub summary: String,
    pub email: Option<String>,
    pub hashed_password: Option<String>,
    pub instance_id: i32
}

#[derive(Insertable)]
#[table_name = "users"]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub is_admin: bool,
    pub summary: String,
    pub email: Option<String>,
    pub hashed_password: Option<String>,
    pub instance_id: i32
}

impl User {
    fn grant_admin_rights() {}

    pub fn insert (conn: &PgConnection, new: NewUser) -> User {
        diesel::insert_into(users::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new instance")
    }

    pub fn compute_outbox(user: String, hostname: String) -> String {
        format!("https://{}/@/{}/outbox", hostname, user)
    }

    pub fn compute_inbox(user: String, hostname: String) -> String {
        format!("https://{}/@/{}/inbox", hostname, user)
    }

    fn get () {}
}
