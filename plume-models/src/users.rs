use activitypub::{
    Activity, Actor, Object, Endpoint, CustomObject,
    actor::Person,
    collection::OrderedCollection,
    object::Image,
};
use bcrypt;
use chrono::{NaiveDateTime, Utc};
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, BelongingToDsl, PgConnection, dsl::any};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign
};
use plume_common::activity_pub::{
    ap_accept_header, ActivityStream, Id, IntoId, ApSignature, PublicKey,
    inbox::WithInbox,
    sign::{Signer, gen_keypair}
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
use webfinger::*;

use {BASE_URL, USE_HTTPS, ap_url};
use db_conn::DbConn;
use blogs::Blog;
use blog_authors::BlogAuthor;
use follows::Follow;
use instance::*;
use likes::Like;
use medias::Media;
use post_authors::PostAuthor;
use posts::Post;
use reshares::Reshare;
use safe_string::SafeString;
use schema::users;

pub const AUTH_COOKIE: &'static str = "user_id";

pub type CustomPerson = CustomObject<ApSignature, Person>;

#[derive(Queryable, Identifiable, Serialize, Deserialize, Clone, Debug)]
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
    pub shared_inbox_url: Option<String>,
    pub followers_endpoint: String,
    pub avatar_id: Option<i32>,
    pub last_fetched_date: NaiveDateTime
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
    pub shared_inbox_url: Option<String>,
    pub followers_endpoint: String,
    pub avatar_id: Option<i32>,
}

const USER_PREFIX: &'static str = "@";

impl User {
    insert!(users, NewUser);
    get!(users);
    find_by!(users, find_by_email, email as String);
    find_by!(users, find_by_name, username as String, instance_id as i32);
    find_by!(users, find_by_ap_url, ap_url as String);

    pub fn one_by_instance(conn: &PgConnection) -> Vec<User> {
        users::table.distinct_on(users::instance_id)
            .get_results::<User>(conn)
            .expect("Error in User::on_by_instance")
    }

    pub fn delete(&self, conn: &PgConnection) {
        diesel::delete(self).execute(conn).expect("Couldn't remove user from DB");
    }

    pub fn get_instance(&self, conn: &PgConnection) -> Instance {
        Instance::get(conn, self.instance_id).expect("Couldn't find instance")
    }

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

    pub fn count_local(conn: &PgConnection) -> usize {
        users::table.filter(users::instance_id.eq(Instance::local_id(conn)))
            .load::<User>(conn)
            .expect("Couldn't load local users")
            .len()
    }

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
        match resolve(acct.clone(), *USE_HTTPS) {
            Ok(wf) => wf.links.into_iter().find(|l| l.mime_type == Some(String::from("application/activity+json"))).and_then(|l| User::fetch_from_url(conn, l.href.expect("No href for AP WF link"))),
            Err(details) => {
                println!("WF Error: {:?}", details);
                None
            }
        }
    }

    fn fetch(url: String) -> Option<CustomPerson> {
        let req = Client::new()
            .get(&url[..])
            .header(Accept(ap_accept_header().into_iter().map(|h| qitem(h.parse::<Mime>().expect("Invalid Content-Type"))).collect()))
            .send();
        match req {
            Ok(mut res) => {
                if let Ok(text) = &res.text() {
                    if let Ok(ap_sign) = serde_json::from_str::<ApSignature>(text) {
                        if let Ok(mut json) = serde_json::from_str::<CustomPerson>(text) {
                            json.custom_props = ap_sign; // without this workaround, publicKey is not correctly deserialized
                            Some(json)
                        } else { None }
                    } else { None }
                } else { None }
            },
            Err(e) => {
                println!("User fetch error: {:?}", e);
                None
            }
        }
    }

    pub fn fetch_from_url(conn: &PgConnection, url: String) -> Option<User> {
        User::fetch(url.clone()).map(|json| (User::from_activity(conn, json, Url::parse(url.as_ref()).unwrap().host_str().unwrap().to_string())))
    }

    fn from_activity(conn: &PgConnection, acct: CustomPerson, inst: String) -> User {
        let instance = match Instance::find_by_domain(conn, inst.clone()) {
            Some(instance) => instance,
            None => {
                Instance::insert(conn, NewInstance {
                    name: inst.clone(),
                    public_domain: inst.clone(),
                    local: false,
                    // We don't really care about all the following for remote instances
                    long_description: SafeString::new(""),
                    short_description: SafeString::new(""),
                    default_license: String::new(),
                    open_registrations: true,
                    short_description_html: String::new(),
                    long_description_html: String::new()
                })
            }
        };

        let avatar = Media::save_remote(conn, acct.object.object_props.icon_image().expect("User::from_activity: icon error")
            .object_props.url_string().expect("User::from_activity: icon.url error"));

        let user = User::insert(conn, NewUser {
            username: acct.object.ap_actor_props.preferred_username_string().expect("User::from_activity: preferredUsername error"),
            display_name: acct.object.object_props.name_string().expect("User::from_activity: name error"),
            outbox_url: acct.object.ap_actor_props.outbox_string().expect("User::from_activity: outbox error"),
            inbox_url: acct.object.ap_actor_props.inbox_string().expect("User::from_activity: inbox error"),
            is_admin: false,
            summary: SafeString::new(&acct.object.object_props.summary_string().unwrap_or(String::new())),
            email: None,
            hashed_password: None,
            instance_id: instance.id,
            ap_url: acct.object.object_props.id_string().expect("User::from_activity: id error"),
            public_key: acct.custom_props.public_key_publickey().expect("User::from_activity: publicKey error")
                .public_key_pem_string().expect("User::from_activity: publicKey.publicKeyPem error"),
            private_key: None,
            shared_inbox_url: acct.object.ap_actor_props.endpoints_endpoint()
                .and_then(|e| e.shared_inbox_string()).ok(),
            followers_endpoint: acct.object.ap_actor_props.followers_string().expect("User::from_activity: followers error"),
            avatar_id: Some(avatar.id)
        });
        avatar.set_owner(conn, user.id);

        user
    }

    pub fn refetch(&self, conn: &PgConnection) {
        User::fetch(self.ap_url.clone()).map(|json| {
            let avatar = Media::save_remote(conn, json.object.object_props.icon_image().expect("User::refetch: icon error")
                .object_props.url_string().expect("User::refetch: icon.url error"));

            diesel::update(self)
                .set((
                    users::username.eq(json.object.ap_actor_props.preferred_username_string().expect("User::refetch: preferredUsername error")),
                    users::display_name.eq(json.object.object_props.name_string().expect("User::refetch: name error")),
                    users::outbox_url.eq(json.object.ap_actor_props.outbox_string().expect("User::refetch: outbox error")),
                    users::inbox_url.eq(json.object.ap_actor_props.inbox_string().expect("User::refetch: inbox error")),
                    users::summary.eq(SafeString::new(&json.object.object_props.summary_string().unwrap_or(String::new()))),
                    users::followers_endpoint.eq(json.object.ap_actor_props.followers_string().expect("User::refetch: followers error")),
                    users::avatar_id.eq(Some(avatar.id)),
                    users::last_fetched_date.eq(Utc::now().naive_utc())
                )).execute(conn)
                .expect("Couldn't update user")
        });
    }

    pub fn hash_pass(pass: String) -> String {
        bcrypt::hash(pass.as_str(), 10).unwrap()
    }

    pub fn auth(&self, pass: String) -> bool {
        if let Ok(valid) = bcrypt::verify(pass.as_str(), self.hashed_password.clone().unwrap().as_str()) {
            valid
        } else {
            false
        }
    }

    pub fn update_boxes(&self, conn: &PgConnection) {
        let instance = self.get_instance(conn);
        if self.outbox_url.len() == 0 {
            diesel::update(self)
                .set(users::outbox_url.eq(instance.compute_box(USER_PREFIX, self.username.clone(), "outbox")))
                .get_result::<User>(conn).expect("Couldn't update outbox URL");
        }

        if self.inbox_url.len() == 0 {
            diesel::update(self)
                .set(users::inbox_url.eq(instance.compute_box(USER_PREFIX, self.username.clone(), "inbox")))
                .get_result::<User>(conn).expect("Couldn't update inbox URL");
        }

        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(users::ap_url.eq(instance.compute_box(USER_PREFIX, self.username.clone(), "")))
                .get_result::<User>(conn).expect("Couldn't update AP URL");
        }

        if self.shared_inbox_url.is_none() {
            diesel::update(self)
                .set(users::shared_inbox_url.eq(ap_url(format!("{}/inbox", Instance::get_local(conn).unwrap().public_domain))))
                .get_result::<User>(conn).expect("Couldn't update shared inbox URL");
        }

        if self.followers_endpoint.len() == 0 {
            diesel::update(self)
                .set(users::followers_endpoint.eq(instance.compute_box(USER_PREFIX, self.username.clone(), "followers")))
                .get_result::<User>(conn).expect("Couldn't update followers endpoint");
        }
    }

    pub fn get_local_page(conn: &PgConnection, (min, max): (i32, i32)) -> Vec<User> {
        users::table.filter(users::instance_id.eq(1))
            .order(users::username.asc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn)
            .expect("Error getting local users page")
    }

    pub fn outbox(&self, conn: &PgConnection) -> ActivityStream<OrderedCollection> {
        let acts = self.get_activities(conn);
        let n_acts = acts.len();
        let mut coll = OrderedCollection::default();
        coll.collection_props.items = serde_json::to_value(acts).unwrap();
        coll.collection_props.set_total_items_u64(n_acts as u64).unwrap();
        ActivityStream::new(coll)
    }

    pub fn fetch_outbox<T: Activity>(&self) -> Vec<T> {
        let req = Client::new()
            .get(&self.outbox_url[..])
            .header(Accept(ap_accept_header().into_iter().map(|h| qitem(h.parse::<Mime>().expect("Invalid Content-Type"))).collect()))
            .send();
        match req {
            Ok(mut res) => {
                let text = &res.text().unwrap();
                let json: serde_json::Value = serde_json::from_str(text).unwrap();
                json["items"].as_array()
                    .expect("Outbox.items is not an array")
                    .into_iter()
                    .filter_map(|j| serde_json::from_value(j.clone()).ok())
                    .collect::<Vec<T>>()
            },
            Err(e) => {
                println!("User outbox fetch error: {:?}", e);
                vec![]
            }
        }
    }

    pub fn fetch_followers_ids(&self) -> Vec<String> {
        let req = Client::new()
            .get(&self.followers_endpoint[..])
            .header(Accept(ap_accept_header().into_iter().map(|h| qitem(h.parse::<Mime>().expect("Invalid Content-Type"))).collect()))
            .send();
        match req {
            Ok(mut res) => {
                let text = &res.text().unwrap();
                let json: serde_json::Value = serde_json::from_str(text).unwrap();
                json["items"].as_array()
                    .expect("Followers.items is not an array")
                    .into_iter()
                    .filter_map(|j| serde_json::from_value(j.clone()).ok())
                    .collect::<Vec<String>>()
            },
            Err(e) => {
                println!("User followers fetch error: {:?}", e);
                vec![]
            }
        }
    }

    fn get_activities(&self, conn: &PgConnection) -> Vec<serde_json::Value> {
        use schema::posts;
        use schema::post_authors;
        let posts_by_self = PostAuthor::belonging_to(self).select(post_authors::post_id);
        let posts = posts::table
            .filter(posts::published.eq(true))
            .filter(posts::id.eq(any(posts_by_self)))
            .load::<Post>(conn).unwrap();
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

    pub fn get_followers_page(&self, conn: &PgConnection, (min, max): (i32, i32)) -> Vec<User> {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table.filter(users::id.eq(any(follows)))
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn).unwrap()
    }

    pub fn get_following(&self, conn: &PgConnection) -> Vec<User> {
        use schema::follows;
        let follows = follows::table.filter(follows::follower_id.eq(self.id)).select(follows::following_id);
        users::table.filter(users::id.eq(any(follows))).load::<User>(conn).unwrap()
    }

    pub fn is_followed_by(&self, conn: &PgConnection, other_id: i32) -> bool {
        use schema::follows;
        follows::table
            .filter(follows::follower_id.eq(other_id))
            .filter(follows::following_id.eq(self.id))
            .load::<Follow>(conn)
            .expect("Couldn't load follow relationship")
            .len() > 0
    }

    pub fn is_following(&self, conn: &PgConnection, other_id: i32) -> bool {
        use schema::follows;
        follows::table
            .filter(follows::follower_id.eq(self.id))
            .filter(follows::following_id.eq(other_id))
            .load::<Follow>(conn)
            .expect("Couldn't load follow relationship")
            .len() > 0
    }

    pub fn has_liked(&self, conn: &PgConnection, post: &Post) -> bool {
        use schema::likes;
        likes::table
            .filter(likes::post_id.eq(post.id))
            .filter(likes::user_id.eq(self.id))
            .load::<Like>(conn)
            .expect("Couldn't load likes")
            .len() > 0
    }

    pub fn has_reshared(&self, conn: &PgConnection, post: &Post) -> bool {
        use schema::reshares;
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

    pub fn into_activity(&self, conn: &PgConnection) -> CustomPerson {
        let mut actor = Person::default();
        actor.object_props.set_id_string(self.ap_url.clone()).expect("User::into_activity: id error");
        actor.object_props.set_name_string(self.display_name.clone()).expect("User::into_activity: name error");
        actor.object_props.set_summary_string(self.summary.get().clone()).expect("User::into_activity: summary error");
        actor.object_props.set_url_string(self.ap_url.clone()).expect("User::into_activity: url error");
        actor.ap_actor_props.set_inbox_string(self.inbox_url.clone()).expect("User::into_activity: inbox error");
        actor.ap_actor_props.set_outbox_string(self.outbox_url.clone()).expect("User::into_activity: outbox error");
        actor.ap_actor_props.set_preferred_username_string(self.username.clone()).expect("User::into_activity: preferredUsername error");
        actor.ap_actor_props.set_followers_string(self.followers_endpoint.clone()).expect("User::into_activity: followers error");

        let mut endpoints = Endpoint::default();
        endpoints.set_shared_inbox_string(ap_url(format!("{}/inbox/", BASE_URL.as_str()))).expect("User::into_activity: endpoints.sharedInbox error");
        actor.ap_actor_props.set_endpoints_endpoint(endpoints).expect("User::into_activity: endpoints error");

        let mut public_key = PublicKey::default();
        public_key.set_id_string(format!("{}#main-key", self.ap_url)).expect("User::into_activity: publicKey.id error");
        public_key.set_owner_string(self.ap_url.clone()).expect("User::into_activity: publicKey.owner error");
        public_key.set_public_key_pem_string(self.public_key.clone()).expect("User::into_activity: publicKey.publicKeyPem error");
        let mut ap_signature = ApSignature::default();
        ap_signature.set_public_key_publickey(public_key).expect("User::into_activity: publicKey error");

        let mut avatar = Image::default();
        avatar.object_props.set_url_string(self.avatar_id.and_then(|id| Media::get(conn, id).map(|m| m.url(conn))).unwrap_or(String::new()))
            .expect("User::into_activity: icon.url error");
        actor.object_props.set_icon_object(avatar).expect("User::into_activity: icon error");

        CustomPerson::new(actor, ap_signature)
    }

    pub fn to_json(&self, conn: &PgConnection) -> serde_json::Value {
        let mut json = serde_json::to_value(self).unwrap();
        json["fqn"] = serde_json::Value::String(self.get_fqn(conn));
        json["name"] = if self.display_name.len() > 0 {
            json!(self.display_name)
        } else {
            json!(self.get_fqn(conn))
        };
        json["avatar"] = json!(self.avatar_id.and_then(|id| Media::get(conn, id).map(|m| m.url(conn))).unwrap_or("/static/default-avatar.png".to_string()));
        json
    }

    pub fn webfinger(&self, conn: &PgConnection) -> Webfinger {
        Webfinger {
            subject: format!("acct:{}@{}", self.username, self.get_instance(conn).public_domain),
            aliases: vec![self.ap_url.clone()],
            links: vec![
                Link {
                    rel: String::from("http://webfinger.net/rel/profile-page"),
                    mime_type: None,
                    href: Some(self.ap_url.clone()),
                    template: None
                },
                Link {
                    rel: String::from("http://schemas.google.com/g/2010#updates-from"),
                    mime_type: Some(String::from("application/atom+xml")),
                    href: Some(self.get_instance(conn).compute_box(USER_PREFIX, self.username.clone(), "feed.atom")),
                    template: None
                },
                Link {
                    rel: String::from("self"),
                    mime_type: Some(String::from("application/activity+json")),
                    href: Some(self.ap_url.clone()),
                    template: None
                }
            ]
        }
    }

    pub fn from_url(conn: &PgConnection, url: String) -> Option<User> {
        User::find_by_ap_url(conn, url.clone()).or_else(|| {
            // The requested user was not in the DB
            // We try to fetch it if it is remote
            if Url::parse(url.as_ref()).unwrap().host_str().unwrap() != BASE_URL.as_str() {
                User::fetch_from_url(conn, url)
            } else {
                None
            }
        })
    }

    pub fn set_avatar(&self, conn: &PgConnection, id: i32) {
        diesel::update(self)
            .set(users::avatar_id.eq(id))
            .execute(conn)
            .expect("Couldn't update user avatar");
    }

    pub fn needs_update(&self) -> bool {
        (Utc::now().naive_utc() - self.last_fetched_date).num_days() > 1
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

    fn is_local(&self) -> bool {
        self.instance_id == 1
    }
}

impl Signer for User {
    fn get_key_id(&self) -> String {
        format!("{}#main-key", self.ap_url)
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
        conn: &PgConnection,
        username: String,
        display_name: String,
        is_admin: bool,
        summary: String,
        email: String,
        password: String
    ) -> User {
        let (pub_key, priv_key) = gen_keypair();
        User::insert(conn, NewUser {
            username: username,
            display_name: display_name,
            outbox_url: String::from(""),
            inbox_url: String::from(""),
            is_admin: is_admin,
            summary: SafeString::new(&summary),
            email: Some(email),
            hashed_password: Some(password),
            instance_id: Instance::local_id(conn),
            ap_url: String::from(""),
            public_key: String::from_utf8(pub_key).unwrap(),
            private_key: Some(String::from_utf8(priv_key).unwrap()),
            shared_inbox_url: None,
            followers_endpoint: String::from(""),
            avatar_id: None
        })
    }
}
