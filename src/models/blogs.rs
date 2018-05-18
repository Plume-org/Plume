use activitystreams_traits::{Actor, Object};
use activitystreams_types::collection::OrderedCollection;
use reqwest::Client;
use reqwest::header::{Accept, qitem};
use reqwest::mime::Mime;
use serde_json;
use url::Url;
use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::sign::Signer;

use activity_pub::{ActivityStream, Id};
use activity_pub::actor::{Actor as APActor, ActorType};
use activity_pub::inbox::WithInbox;
use activity_pub::sign;
use activity_pub::webfinger::*;
use models::instance::Instance;
use schema::blogs;


#[derive(Queryable, Identifiable, Serialize, Deserialize, Clone)]
pub struct Blog {
    pub id: i32,
    pub actor_id: String,
    pub title: String,
    pub summary: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub instance_id: i32,
    pub creation_date: NaiveDateTime,
    pub ap_url: String,
    pub private_key: Option<String>,
    pub public_key: String
}

#[derive(Insertable)]
#[table_name = "blogs"]
pub struct NewBlog {
    pub actor_id: String,
    pub title: String,
    pub summary: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub instance_id: i32,
    pub ap_url: String,
    pub private_key: Option<String>,
    pub public_key: String
}

impl Blog {
    pub fn insert (conn: &PgConnection, new: NewBlog) -> Blog {
        diesel::insert_into(blogs::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new blog")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Blog> {
        blogs::table.filter(blogs::id.eq(id))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Error loading blog by id")
            .into_iter().nth(0)
    }

    pub fn find_by_name(conn: &PgConnection, name: String, instance_id: i32) -> Option<Blog> {
        blogs::table.filter(blogs::actor_id.eq(name))
            .filter(blogs::instance_id.eq(instance_id))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Error loading blog by name")
            .into_iter().nth(0)
    }

    pub fn find_local(conn: &PgConnection, name: String) -> Option<Blog> {
        Blog::find_by_name(conn, name, Instance::local_id(conn))
    }

    pub fn find_by_fqn(conn: &PgConnection, fqn: String) -> Option<Blog> {
        if fqn.contains("@") { // remote blog
            match Instance::find_by_domain(conn, String::from(fqn.split("@").last().unwrap())) {
                Some(instance) => {
                    match Blog::find_by_name(conn, String::from(fqn.split("@").nth(0).unwrap()), instance.id) {
                        Some(u) => Some(u),
                        None => Blog::fetch_from_webfinger(conn, fqn)
                    }
                },
                None => Blog::fetch_from_webfinger(conn, fqn)
            }
        } else { // local blog
            Blog::find_local(conn, fqn)
        }
    }

    fn fetch_from_webfinger(conn: &PgConnection, acct: String) -> Option<Blog> {
        match resolve(acct.clone()) {
            Ok(url) => Blog::fetch_from_url(conn, url),
            Err(details) => {
                println!("{}", details);
                None
            }
        }
    }

    fn fetch_from_url(conn: &PgConnection, url: String) -> Option<Blog> {
        let req = Client::new()
            .get(&url[..])
            .header(Accept(vec![qitem("application/activity+json".parse::<Mime>().unwrap())]))
            .send();
        match req {
            Ok(mut res) => {
                let json: serde_json::Value = serde_json::from_str(&res.text().unwrap()).unwrap();
                Some(Blog::from_activity(conn, json, Url::parse(url.as_ref()).unwrap().host_str().unwrap().to_string()))
            },
            Err(_) => None
        }
    }

    fn from_activity(conn: &PgConnection, acct: serde_json::Value, inst: String) -> Blog {
        let instance = match Instance::find_by_domain(conn, inst.clone()) {
            Some(instance) => instance,
            None => {
                Instance::insert(conn, inst.clone(), inst.clone(), false)
            }
        };
        Blog::insert(conn, NewBlog {
            actor_id: acct["preferredUsername"].as_str().unwrap().to_string(),
            title: acct["name"].as_str().unwrap().to_string(),
            outbox_url: acct["outbox"].as_str().unwrap().to_string(),
            inbox_url: acct["inbox"].as_str().unwrap().to_string(),
            summary: acct["summary"].as_str().unwrap().to_string(),
            instance_id: instance.id,
            ap_url: acct["id"].as_str().unwrap().to_string(),
            public_key: acct["publicKey"]["publicKeyPem"].as_str().unwrap_or("").to_string(),
            private_key: None
        })
    }

    pub fn update_boxes(&self, conn: &PgConnection) {
        if self.outbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::outbox_url.eq(self.compute_outbox(conn)))
                .get_result::<Blog>(conn).expect("Couldn't update outbox URL");
        }

        if self.inbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::inbox_url.eq(self.compute_inbox(conn)))
                .get_result::<Blog>(conn).expect("Couldn't update inbox URL");
        }

        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(blogs::ap_url.eq(self.compute_id(conn)))
                .get_result::<Blog>(conn).expect("Couldn't update AP URL");
        }
    }

    pub fn outbox(&self, conn: &PgConnection) -> ActivityStream<OrderedCollection> {
        let mut coll = OrderedCollection::default();
        coll.collection_props.items = serde_json::to_value(self.get_activities(conn)).unwrap();
        coll.collection_props.set_total_items_u64(self.get_activities(conn).len() as u64).unwrap();
        ActivityStream::new(coll)
    }

    fn get_activities(&self, _conn: &PgConnection) -> Vec<serde_json::Value> {
        vec![]
    }

    pub fn get_keypair(&self) -> PKey<Private> {
        PKey::from_rsa(Rsa::private_key_from_pem(self.private_key.clone().unwrap().as_ref()).unwrap()).unwrap()
    }
}

impl Into<Id> for Blog {
    fn into(self) -> Id {
        Id::new(self.ap_url)
    }
}

impl Object for Blog {}
impl Actor for Blog {}

impl WithInbox for Blog {
    fn get_inbox_url(&self) -> String {
        self.inbox_url.clone()
    }

    fn get_shared_inbox_url(&self) -> Option<String> {
        None
    }
}

impl APActor for Blog {
    fn get_box_prefix() -> &'static str {
        "~"
    }

    fn get_actor_id(&self) -> String {
        self.actor_id.to_string()
    }

    fn get_display_name(&self) -> String {
        self.title.clone()
    }

    fn get_summary(&self) -> String {
        self.summary.clone()
    }

    fn get_instance(&self, conn: &PgConnection) -> Instance {
        Instance::get(conn, self.instance_id).unwrap()
    }

    fn get_actor_type () -> ActorType {
        ActorType::Blog
    }

    fn get_inbox_url(&self) -> String {
        self.inbox_url.clone()
    }

    fn get_shared_inbox_url(&self) -> Option<String> {
        None
    }

    fn from_url(conn: &PgConnection, url: String) -> Option<Blog> {
        blogs::table.filter(blogs::ap_url.eq(url))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Error loading blog from url")
            .into_iter().nth(0)
    }
}

impl Webfinger for Blog {
    fn webfinger_subject(&self, conn: &PgConnection) -> String {
        format!("acct:{}@{}", self.actor_id, self.get_instance(conn).public_domain)
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

impl sign::Signer for Blog {
    fn get_key_id(&self, conn: &PgConnection) -> String {
        format!("{}#main-key", self.compute_id(conn))
    }

    fn sign(&self, to_sign: String) -> Vec<u8> {
        let key = self.get_keypair();
        let mut signer = Signer::new(MessageDigest::sha256(), &key).unwrap();
        signer.update(to_sign.as_bytes()).unwrap();
        signer.sign_to_vec().unwrap()
    }
}

impl NewBlog {
    pub fn new_local(
        actor_id: String,
        title: String,
        summary: String,
        instance_id: i32
    ) -> NewBlog {
        let (pub_key, priv_key) = sign::gen_keypair();
        NewBlog {
            actor_id: actor_id,
            title: title,
            summary: summary,
            outbox_url: String::from(""),
            inbox_url: String::from(""),
            instance_id: instance_id,
            ap_url: String::from(""),
            public_key: String::from_utf8(pub_key).unwrap(),
            private_key: Some(String::from_utf8(priv_key).unwrap())
        }
    }
}
