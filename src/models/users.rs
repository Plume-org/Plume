use bcrypt;
use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, BelongingToDsl, PgConnection};
use diesel::dsl::any;
use reqwest::Client;
use reqwest::header::{Accept, qitem};
use reqwest::mime::Mime;
use rocket::request::{self, FromRequest, Request};
use rocket::outcome::IntoOutcome;
use serde_json;
use url::Url;

use BASE_URL;
use activity_pub::activity::Activity;
use activity_pub::actor::{ActorType, Actor};
use activity_pub::inbox::Inbox;
use activity_pub::outbox::Outbox;
use activity_pub::webfinger::{Webfinger, resolve};
use db_conn::DbConn;
use models::follows::Follow;
use models::instance::Instance;
use models::post_authors::PostAuthor;
use models::posts::Post;
use schema::users;

pub const AUTH_COOKIE: &'static str = "user_id";

#[derive(Queryable, Identifiable, Serialize)]
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
    pub instance_id: i32,
    pub creation_date: NaiveDateTime,
    pub ap_url: String
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
    pub instance_id: i32,
    pub ap_url: String
}

impl User {
    pub fn grant_admin_rights() {}

    pub fn insert (conn: &PgConnection, new: NewUser) -> User {
        diesel::insert_into(users::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new user")
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

    pub fn find_by_name(conn: &PgConnection, username: String, instance_id: i32) -> Option<User> {
        users::table.filter(users::username.eq(username))
            .filter(users::instance_id.eq(instance_id))
            .limit(1)
            .load::<User>(conn)
            .expect("Error loading user by name")
            .into_iter().nth(0)
    }

    pub fn find_local(conn: &PgConnection, username: String) -> Option<User> {
        User::find_by_name(conn, username, Instance::local_id(conn))
    }

    pub fn find_by_fqn(conn: &PgConnection, fqn: String) -> Option<User> {
        if fqn.contains("@") { // remote user
            match Instance::get_by_domain(conn, String::from(fqn.split("@").last().unwrap())) {
                Some(instance) => {
                    match User::find_by_name(conn, String::from(fqn.split("@").nth(0).unwrap()), instance.id) {
                        Some(u) => Some(u),
                        None => User::fetch_from_webfinger(conn, fqn)
                    }
                },
                None => User::fetch_from_webfinger(conn, fqn)
            }
        } else { // local user
            User::find_local(conn, fqn)
        }
    }

    fn fetch_from_webfinger(conn: &PgConnection, acct: String) -> Option<User> {
        match resolve(acct.clone()) {
            Ok(url) => User::fetch_from_url(conn, url),
            Err(details) => {
                println!("{}", details);
                None
            }
        }
    }

    fn fetch_from_url(conn: &PgConnection, url: String) -> Option<User> {
        let req = Client::new()
            .get(&url[..])
            .header(Accept(vec![qitem("application/activity+json".parse::<Mime>().unwrap())]))
            .send();
        match req {
            Ok(mut res) => {
                let json: serde_json::Value = serde_json::from_str(&res.text().unwrap()).unwrap();
                Some(User::from_activity(conn, json, Url::parse(url.as_ref()).unwrap().host_str().unwrap().to_string()))
            },
            Err(_) => None
        }
    }

    fn from_activity(conn: &PgConnection, acct: serde_json::Value, inst: String) -> User {
        let instance = match Instance::get_by_domain(conn, inst.clone()) {
            Some(instance) => instance,
            None => {
                Instance::insert(conn, inst.clone(), inst.clone(), false)
            }
        };
        User::insert(conn, NewUser {
            username: acct["preferredUsername"].as_str().unwrap().to_string(),
            display_name: acct["name"].as_str().unwrap().to_string(),
            outbox_url: acct["outbox"].as_str().unwrap().to_string(),
            inbox_url: acct["inbox"].as_str().unwrap().to_string(),
            is_admin: false,
            summary: acct["summary"].as_str().unwrap().to_string(),
            email: None,
            hashed_password: None,
            instance_id: instance.id,
            ap_url: acct["id"].as_str().unwrap().to_string()
        })
    }

    pub fn hash_pass(pass: String) -> String {
        bcrypt::hash(pass.as_str(), bcrypt::DEFAULT_COST).unwrap()
    }

    pub fn auth(&self, pass: String) -> bool {
        bcrypt::verify(pass.as_str(), self.hashed_password.clone().unwrap().as_str()).is_ok()
    }

    pub fn update_boxes(&self, conn: &PgConnection) {
        if self.outbox_url.len() == 0 {
            diesel::update(self)
                .set(users::outbox_url.eq(self.compute_outbox(conn)))
                .get_result::<User>(conn).expect("Couldn't update outbox URL");                
        }

        if self.inbox_url.len() == 0 {
            diesel::update(self)
                .set(users::inbox_url.eq(self.compute_inbox(conn)))
                .get_result::<User>(conn).expect("Couldn't update inbox URL");                
        }

        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(users::ap_url.eq(self.compute_id(conn)))
                .get_result::<User>(conn).expect("Couldn't update AP URL");
        }
    }

    pub fn outbox(&self, conn: &PgConnection) -> Outbox {
        Outbox::new(self.compute_outbox(conn), self.get_activities(conn))
    }

    fn get_activities(&self, conn: &PgConnection) -> Vec<Activity> {
        use schema::posts;
        use schema::post_authors;
        let posts_by_self = PostAuthor::belonging_to(self).select(post_authors::post_id);
        let posts = posts::table.filter(posts::id.eq(any(posts_by_self))).load::<Post>(conn).unwrap();
        posts.into_iter().map(|p| Activity::create(self, p, conn)).collect::<Vec<Activity>>()
    }

    pub fn get_followers(&self, conn: &PgConnection) -> Vec<User> {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table.filter(users::id.eq(any(follows))).load::<User>(conn).unwrap()
    }

    pub fn get_following(&self, conn: &PgConnection) -> Vec<User> {
        use schema::follows;
        let follows = follows::table.filter(follows::follower_id.eq(self.id)).select(follows::following_id);
        users::table.filter(users::id.eq(any(follows))).load::<User>(conn).unwrap()
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

    fn get_actor_type() -> ActorType {
        ActorType::Person
    }

    fn from_url(conn: &PgConnection, url: String) -> Option<User> {
        let in_db = users::table.filter(users::ap_url.eq(url.clone()))
            .limit(1)
            .load::<User>(conn)
            .expect("Error loading user by AP url")
            .into_iter().nth(0);
        match in_db {
            Some(u) => Some(u),
            None => {
                // The requested user was not in the DB
                // We try to fetch it if it is remote
                if Url::parse(url.as_ref()).unwrap().host_str().unwrap() != BASE_URL.as_str() {
                    Some(User::fetch_from_url(conn, url).unwrap())
                } else {
                    None
                }
            }
        }
    }
}

impl Inbox for User {
    fn received(&self, conn: &PgConnection, act: serde_json::Value) {
        self.save(conn, act);
        // TODO: add to stream or create notification, or whatever needs to be done
    }
}

impl Webfinger for User {
    fn webfinger_subject(&self, conn: &PgConnection) -> String {
        format!("acct:{}@{}", self.username, self.get_instance(conn).public_domain)
    }
    fn webfinger_aliases(&self, conn: &PgConnection) -> Vec<String> {
        vec![self.compute_id(conn)]
    }
    fn webfinger_links(&self, conn: &PgConnection) -> Vec<Vec<(String, String)>> {
        vec![
            vec![
                (String::from("rel"), String::from("http://webfinger.net/rel/profile-page")),
                (String::from("href"), self.compute_id(conn))
            ],
            vec![
                (String::from("rel"), String::from("http://schemas.google.com/g/2010#updates-from")),
                (String::from("type"), String::from("application/atom+xml")),
                (String::from("href"), self.compute_box(conn, "feed.atom"))
            ],
            vec![
                (String::from("rel"), String::from("self")),
                (String::from("type"), String::from("application/activity+json")),
                (String::from("href"), self.compute_id(conn))
            ]
        ]
    }
}

impl NewUser {
    /// Creates a new local user
    pub fn new_local(
        username: String,
        display_name: String,
        is_admin: bool,
        summary: String,
        email: String,
        password: String,
        instance_id: i32
    ) -> NewUser {
        NewUser {
            username: username,
            display_name: display_name,
            outbox_url: String::from(""),
            inbox_url: String::from(""),
            is_admin: is_admin,
            summary: summary,
            email: Some(email),
            hashed_password: Some(password),
            instance_id: instance_id,
            ap_url: String::from("")
        }
    }
}
