use activitypub::{
    Actor, Object,
    actor::{Person, properties::ApActorProperties},
    collection::OrderedCollection,
    object::properties::ObjectProperties
};
use bcrypt;
use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, BelongingToDsl, PgConnection, dsl::any};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign
};
use reqwest::{
    Client,
    header::{Accept, qitem},
    mime::Mime
};
use rocket::{
    request::{self, FromRequest, Request},
    outcome::IntoOutcome
};
use serde_json;
use url::Url;

use BASE_URL;
use activity_pub::{
    ap_url, ActivityStream, Id, IntoId,
    actor::{ActorType, Actor as APActor},
    inbox::{Inbox, WithInbox},
    sign::{Signer, gen_keypair},
    webfinger::{Webfinger, resolve}
};
use db_conn::DbConn;
use models::{
    blogs::Blog,
    blog_authors::BlogAuthor,
    follows::Follow,
    instance::*,
    post_authors::PostAuthor,
    posts::Post
};
use schema::users;
use safe_string::SafeString;

pub const AUTH_COOKIE: &'static str = "user_id";

#[derive(Queryable, Identifiable, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub is_admin: bool,
    pub summary: SafeString,
    pub email: Option<String>,
    pub hashed_password: Option<String>,
    pub instance_id: i32,
    pub creation_date: NaiveDateTime,
    pub ap_url: String,
    pub private_key: Option<String>,
    pub public_key: String,
    pub shared_inbox_url: Option<String>
}

#[derive(Insertable)]
#[table_name = "users"]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub is_admin: bool,
    pub summary: SafeString,
    pub email: Option<String>,
    pub hashed_password: Option<String>,
    pub instance_id: i32,
    pub ap_url: String,
    pub private_key: Option<String>,
    pub public_key: String,
    pub shared_inbox_url: Option<String>    
}

impl User {
    insert!(users, NewUser);

    pub fn grant_admin_rights(&self, conn: &PgConnection) {
        diesel::update(self)
            .set(users::is_admin.eq(true))
            .load::<User>(conn)
            .expect("Couldn't grant admin rights");
    }

    pub fn update(&self, conn: &PgConnection, name: String, email: String, summary: String) -> User {
        diesel::update(self)
            .set((
                users::display_name.eq(name),
                users::email.eq(email),
                users::summary.eq(summary),
            )).load::<User>(conn)
            .expect("Couldn't update user")
            .into_iter().nth(0).unwrap()
    }

    get!(users);

    pub fn count_local(conn: &PgConnection) -> usize {
        users::table.filter(users::instance_id.eq(Instance::local_id(conn)))
            .load::<User>(conn)
            .expect("Couldn't load local users")
            .len()
    }

    find_by!(users, find_by_email, email as String);
    find_by!(users, find_by_name, username as String, instance_id as i32);

    pub fn find_local(conn: &PgConnection, username: String) -> Option<User> {
        User::find_by_name(conn, username, Instance::local_id(conn))
    }

    pub fn find_by_fqn(conn: &PgConnection, fqn: String) -> Option<User> {
        if fqn.contains("@") { // remote user
            match Instance::find_by_domain(conn, String::from(fqn.split("@").last().unwrap())) {
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
        let instance = match Instance::find_by_domain(conn, inst.clone()) {
            Some(instance) => instance,
            None => {
                Instance::insert(conn, NewInstance {
                    name: inst.clone(),
                    public_domain: inst.clone(),
                    local: false
                })
            }
        };
        User::insert(conn, NewUser {
            username: acct["preferredUsername"].as_str().unwrap().to_string(),
            display_name: acct["name"].as_str().unwrap().to_string(),
            outbox_url: acct["outbox"].as_str().unwrap().to_string(),
            inbox_url: acct["inbox"].as_str().unwrap().to_string(),
            is_admin: false,
            summary: SafeString::new(&acct["summary"].as_str().unwrap().to_string()),
            email: None,
            hashed_password: None,
            instance_id: instance.id,
            ap_url: acct["id"].as_str().unwrap().to_string(),
            public_key: acct["publicKey"]["publicKeyPem"].as_str().unwrap().to_string(),
            private_key: None,
            shared_inbox_url: acct["endpoints"]["sharedInbox"].as_str().map(|s| s.to_string())
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

        if self.shared_inbox_url.is_none() {
            diesel::update(self)
                .set(users::shared_inbox_url.eq(ap_url(format!("{}/inbox", Instance::get_local(conn).unwrap().public_domain))))
                .get_result::<User>(conn).expect("Couldn't update shared inbox URL");
        }
    }

    pub fn outbox(&self, conn: &PgConnection) -> ActivityStream<OrderedCollection> {
        let acts = self.get_activities(conn);
        let n_acts = acts.len();
        let mut coll = OrderedCollection::default();
        coll.collection_props.items = serde_json::to_value(acts).unwrap();
        coll.collection_props.set_total_items_u64(n_acts as u64).unwrap();
        ActivityStream::new(coll)
    }

    fn get_activities(&self, conn: &PgConnection) -> Vec<serde_json::Value> {
        use schema::posts;
        use schema::post_authors;
        let posts_by_self = PostAuthor::belonging_to(self).select(post_authors::post_id);
        let posts = posts::table.filter(posts::id.eq(any(posts_by_self))).load::<Post>(conn).unwrap();
        posts.into_iter().map(|p| {
            serde_json::to_value(p.create_activity(conn)).unwrap()
        }).collect::<Vec<serde_json::Value>>()
    }

    pub fn get_fqn(&self, conn: &PgConnection) -> String {
        if self.instance_id == Instance::local_id(conn) {
            self.username.clone()
        } else {
            format!("{}@{}", self.username, self.get_instance(conn).public_domain)
        }
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

    pub fn is_following(&self, conn: &PgConnection, other_id: i32) -> bool {
        use schema::follows;
        follows::table
            .filter(follows::follower_id.eq(other_id))
            .filter(follows::following_id.eq(self.id))
            .load::<Follow>(conn)
            .expect("Couldn't load follow relationship")
            .len() > 0
    }

    pub fn has_liked(&self, conn: &PgConnection, post: &Post) -> bool {
        use schema::likes;
        use models::likes::Like;
        likes::table
            .filter(likes::post_id.eq(post.id))
            .filter(likes::user_id.eq(self.id))
            .load::<Like>(conn)
            .expect("Couldn't load likes")
            .len() > 0
    }

    pub fn has_reshared(&self, conn: &PgConnection, post: &Post) -> bool {
        use schema::reshares;
        use models::reshares::Reshare;
        reshares::table
            .filter(reshares::post_id.eq(post.id))
            .filter(reshares::user_id.eq(self.id))
            .load::<Reshare>(conn)
            .expect("Couldn't load reshares")
            .len() > 0
    }

    pub fn is_author_in(&self, conn: &PgConnection, blog: Blog) -> bool {
        use schema::blog_authors;
        blog_authors::table.filter(blog_authors::author_id.eq(self.id))
            .filter(blog_authors::blog_id.eq(blog.id))
            .load::<BlogAuthor>(conn)
            .expect("Couldn't load blog/author relationship")
            .len() > 0
    }

    pub fn get_keypair(&self) -> PKey<Private> {
        PKey::from_rsa(Rsa::private_key_from_pem(self.private_key.clone().unwrap().as_ref()).unwrap()).unwrap()
    }

    pub fn into_activity(&self, conn: &PgConnection) -> Person {
        let mut actor = Person::default();
        actor.object_props = ObjectProperties {
            id: Some(serde_json::to_value(self.compute_id(conn)).unwrap()),
            name: Some(serde_json::to_value(self.get_display_name()).unwrap()),
            summary: Some(serde_json::to_value(self.get_summary()).unwrap()),
            url: Some(serde_json::to_value(self.compute_id(conn)).unwrap()),
            ..ObjectProperties::default()
        };
        actor.ap_actor_props = ApActorProperties {
            inbox: serde_json::to_value(self.compute_inbox(conn)).unwrap(),
            outbox: serde_json::to_value(self.compute_outbox(conn)).unwrap(),
            preferred_username: Some(serde_json::to_value(self.get_actor_id()).unwrap()),
            endpoints: Some(json!({
                "sharedInbox": ap_url(format!("{}/inbox", BASE_URL.as_str()))
            })),
            followers: None,
            following: None,
            liked: None,
            streams: None
        };
        actor
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

impl APActor for User {
    fn get_box_prefix() -> &'static str {
        "@"
    }

    fn get_actor_id(&self) -> String {
        self.username.to_string()
    }

    fn get_display_name(&self) -> String {
        self.display_name.clone()
    }

    fn get_summary(&self) -> String {
        self.summary.get().clone()
    }

    fn get_instance(&self, conn: &PgConnection) -> Instance {
        Instance::get(conn, self.instance_id).unwrap()
    }

    fn get_actor_type() -> ActorType {
        ActorType::Person
    }

    fn get_inbox_url(&self) -> String {
        self.inbox_url.clone()
    }

    fn get_shared_inbox_url(&self) -> Option<String> {
       self.shared_inbox_url.clone()
    }

    fn custom_props(&self, conn: &PgConnection) -> serde_json::Map<String, serde_json::Value> {
        let mut res = serde_json::Map::new();
        res.insert("publicKey".to_string(), json!({
            "id": self.get_key_id(conn),
            "owner": self.compute_id(conn),
            "publicKeyPem": self.public_key
        }));
        res.insert("followers".to_string(), serde_json::Value::String(self.compute_box(conn, "followers")));
        res
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

impl IntoId for User {
    fn into_id(self) -> Id {
        Id::new(self.ap_url.clone())
    }
}

impl Object for User {}
impl Actor for User {}

impl WithInbox for User {
    fn get_inbox_url(&self) -> String {
        self.inbox_url.clone()
    }

    fn get_shared_inbox_url(&self) -> Option<String> {
       self.shared_inbox_url.clone()
    }
}

impl Inbox for User {
    fn received(&self, conn: &PgConnection, act: serde_json::Value) {
        if let Err(err) = self.save(conn, act.clone()) {
            println!("Inbox error:\n{}\n{}\n\nActivity was: {}", err.cause(), err.backtrace(), act.to_string());
        }

        // TODO: add to stream, or whatever needs to be done
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

impl Signer for User {
    fn get_key_id(&self, conn: &PgConnection) -> String {
        format!("{}#main-key", self.compute_id(conn))
    }

    fn sign(&self, to_sign: String) -> Vec<u8> {
        let key = self.get_keypair();
        let mut signer = sign::Signer::new(MessageDigest::sha256(), &key).unwrap();
        signer.update(to_sign.as_bytes()).unwrap();
        signer.sign_to_vec().unwrap()
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
        let (pub_key, priv_key) = gen_keypair();
        NewUser {
            username: username,
            display_name: display_name,
            outbox_url: String::from(""),
            inbox_url: String::from(""),
            is_admin: is_admin,
            summary: SafeString::new(&summary),
            email: Some(email),
            hashed_password: Some(password),
            instance_id: instance_id,
            ap_url: String::from(""),
            public_key: String::from_utf8(pub_key).unwrap(),
            private_key: Some(String::from_utf8(priv_key).unwrap()),
            shared_inbox_url: None
        }
    }
}
