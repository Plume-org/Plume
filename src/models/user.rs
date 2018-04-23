use rocket::request;
use rocket::request::{FromRequest, Request};
use rocket::outcome::IntoOutcome;
use diesel;
use diesel::{QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use schema::users;
use db_conn::DbConn;
use activity_pub::Actor;
use models::instance::Instance;
use bcrypt;

pub const AUTH_COOKIE: &'static str = "user_id";

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
    pub fn grant_admin_rights() {}

    pub fn insert (conn: &PgConnection, new: NewUser) -> User {
        diesel::insert_into(users::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new user")
    }

    pub fn compute_outbox(user: String, hostname: String) -> String {
        format!("https://{}/@/{}/outbox", hostname, user)
    }

    pub fn compute_inbox(user: String, hostname: String) -> String {
        format!("https://{}/@/{}/inbox", hostname, user)
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<User> {
        users::table.filter(users::id.eq(id))
            .limit(1)
            .load::<User>(conn)
            .expect("Error loading user by id")
            .into_iter().nth(0)
    }

    pub fn find_by_email(conn: &PgConnection, email: String) -> Option<User> {
        users::table.filter(users::email.eq(email))
            .limit(1)
            .load::<User>(conn)
            .expect("Error loading user by email")
            .into_iter().nth(0)
    }

    pub fn find_by_name(conn: &PgConnection, username: String) -> Option<User> {
        users::table.filter(users::username.eq(username))
            .limit(1)
            .load::<User>(conn)
            .expect("Error loading user by email")
            .into_iter().nth(0)
    }

    pub fn hash_pass(pass: String) -> String {
        bcrypt::hash(pass.as_str(), bcrypt::DEFAULT_COST).unwrap()
    }

    pub fn auth(&self, pass: String) -> bool {
        bcrypt::verify(pass.as_str(), self.hashed_password.clone().unwrap().as_str()).is_ok()
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<User, ()> {
        let conn = request.guard::<DbConn>()?;
        request.cookies()
            .get_private(AUTH_COOKIE)
            .and_then(|cookie| cookie.value().parse().ok())
            .map(|id| User::get(&*conn, id).unwrap())
            .or_forward(())
    }
}

impl Actor for User {
    fn get_box_prefix() -> &'static str {
        "@"
    }

    fn get_actor_id(&self) -> String {
        self.username.to_string()
    }

    fn get_instance(&self, conn: &PgConnection) -> Instance {
        Instance::get(conn, self.instance_id).unwrap()
    }
}
