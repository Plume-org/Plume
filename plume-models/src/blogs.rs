use activitypub::{Actor, Object, CustomObject, actor::Group, collection::OrderedCollection};
use reqwest::{
    Client,
    header::{Accept, qitem},
    mime::Mime
};
use serde_json;
use url::Url;
use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection, dsl::any};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign::Signer
};
use webfinger::*;

use {BASE_URL, USE_HTTPS};
use plume_common::activity_pub::{
    ap_accept_header, ApSignature, ActivityStream, Id, IntoId, PublicKey,
    inbox::WithInbox,
    sign
};
use safe_string::SafeString;
use instance::*;
use users::User;
use schema::blogs;

pub type CustomGroup = CustomObject<ApSignature, Group>;

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

const BLOG_PREFIX: &'static str = "~";

impl Blog {
    insert!(blogs, NewBlog);
    get!(blogs);
    find_by!(blogs, find_by_ap_url, ap_url as String);
    find_by!(blogs, find_by_name, actor_id as String, instance_id as i32);

    pub fn get_instance(&self, conn: &PgConnection) -> Instance {
        Instance::get(conn, self.instance_id).expect("Couldn't find instance")
    }

    pub fn list_authors(&self, conn: &PgConnection) -> Vec<User> {
        use schema::blog_authors;
        use schema::users;
        let authors_ids = blog_authors::table.filter(blog_authors::blog_id.eq(self.id)).select(blog_authors::author_id);
        users::table.filter(users::id.eq(any(authors_ids)))
            .load::<User>(conn)
            .expect("Couldn't load authors of a blog")
    }

    pub fn find_for_author(conn: &PgConnection, author_id: i32) -> Vec<Blog> {
        use schema::blog_authors;
        let author_ids = blog_authors::table.filter(blog_authors::author_id.eq(author_id)).select(blog_authors::blog_id);
        blogs::table.filter(blogs::id.eq(any(author_ids)))
            .load::<Blog>(conn)
            .expect("Couldn't load blogs ")
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
        match resolve(acct.clone(), *USE_HTTPS) {
            Ok(wf) => wf.links.into_iter().find(|l| l.mime_type == Some(String::from("application/activity+json"))).and_then(|l| Blog::fetch_from_url(conn, l.href.expect("No href for AP WF link"))),
            Err(details) => {
                println!("{:?}", details);
                None
            }
        }
    }

    fn fetch_from_url(conn: &PgConnection, url: String) -> Option<Blog> {
        let req = Client::new()
            .get(&url[..])
            .header(Accept(ap_accept_header().into_iter().map(|h| qitem(h.parse::<Mime>().expect("Invalid Content-Type"))).collect()))
            .send();
        match req {
            Ok(mut res) => {
                let text = &res.text().unwrap();
                let ap_sign: ApSignature = serde_json::from_str(text).unwrap();
                let mut json: CustomGroup = serde_json::from_str(text).unwrap();
                json.custom_props = ap_sign; // without this workaround, publicKey is not correctly deserialized
                Some(Blog::from_activity(conn, json, Url::parse(url.as_ref()).unwrap().host_str().unwrap().to_string()))
            },
            Err(_) => None
        }
    }

    fn from_activity(conn: &PgConnection, acct: CustomGroup, inst: String) -> Blog {
        let instance = match Instance::find_by_domain(conn, inst.clone()) {
            Some(instance) => instance,
            None => {
                Instance::insert(conn, NewInstance {
                    public_domain: inst.clone(),
                    name: inst.clone(),
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
        Blog::insert(conn, NewBlog {
            actor_id: acct.object.ap_actor_props.preferred_username_string().expect("Blog::from_activity: preferredUsername error"),
            title: acct.object.object_props.name_string().expect("Blog::from_activity: name error"),
            outbox_url: acct.object.ap_actor_props.outbox_string().expect("Blog::from_activity: outbox error"),
            inbox_url: acct.object.ap_actor_props.inbox_string().expect("Blog::from_activity: inbox error"),
            summary: acct.object.object_props.summary_string().expect("Blog::from_activity: summary error"),
            instance_id: instance.id,
            ap_url: acct.object.object_props.id_string().expect("Blog::from_activity: id error"),
            public_key: acct.custom_props.public_key_publickey().expect("Blog::from_activity: publicKey error")
                .public_key_pem_string().expect("Blog::from_activity: publicKey.publicKeyPem error"),
            private_key: None
        })
    }

    pub fn into_activity(&self, _conn: &PgConnection) -> CustomGroup {
        let mut blog = Group::default();
        blog.ap_actor_props.set_preferred_username_string(self.actor_id.clone()).expect("Blog::into_activity: preferredUsername error");
        blog.object_props.set_name_string(self.title.clone()).expect("Blog::into_activity: name error");
        blog.ap_actor_props.set_outbox_string(self.outbox_url.clone()).expect("Blog::into_activity: outbox error");
        blog.ap_actor_props.set_inbox_string(self.inbox_url.clone()).expect("Blog::into_activity: inbox error");
        blog.object_props.set_summary_string(self.summary.clone()).expect("Blog::into_activity: summary error");
        blog.object_props.set_id_string(self.ap_url.clone()).expect("Blog::into_activity: id error");

        let mut public_key = PublicKey::default();
        public_key.set_id_string(format!("{}#main-key", self.ap_url)).expect("Blog::into_activity: publicKey.id error");
        public_key.set_owner_string(self.ap_url.clone()).expect("Blog::into_activity: publicKey.owner error");
        public_key.set_public_key_pem_string(self.public_key.clone()).expect("Blog::into_activity: publicKey.publicKeyPem error");
        let mut ap_signature = ApSignature::default();
        ap_signature.set_public_key_publickey(public_key).expect("Blog::into_activity: publicKey error");

        CustomGroup::new(blog, ap_signature)
    }

    pub fn update_boxes(&self, conn: &PgConnection) {
        let instance = self.get_instance(conn);
        if self.outbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::outbox_url.eq(instance.compute_box(BLOG_PREFIX, self.actor_id.clone(), "outbox")))
                .get_result::<Blog>(conn).expect("Couldn't update outbox URL");
        }

        if self.inbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::inbox_url.eq(instance.compute_box(BLOG_PREFIX, self.actor_id.clone(), "inbox")))
                .get_result::<Blog>(conn).expect("Couldn't update inbox URL");
        }

        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(blogs::ap_url.eq(instance.compute_box(BLOG_PREFIX, self.actor_id.clone(), "")))
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

    pub fn webfinger(&self, conn: &PgConnection) -> Webfinger {
        Webfinger {
            subject: format!("acct:{}@{}", self.actor_id, self.get_instance(conn).public_domain),
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
                    href: Some(self.get_instance(conn).compute_box(BLOG_PREFIX, self.actor_id.clone(), "feed.atom")),
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

    pub fn from_url(conn: &PgConnection, url: String) -> Option<Blog> {
        Blog::find_by_ap_url(conn, url.clone()).or_else(|| {
            // The requested user was not in the DB
            // We try to fetch it if it is remote
            if Url::parse(url.as_ref()).unwrap().host_str().unwrap() != BASE_URL.as_str() {
                Blog::fetch_from_url(conn, url)
            } else {
                None
            }
        })
    }

    pub fn get_fqn(&self, conn: &PgConnection) -> String {
        if self.instance_id == Instance::local_id(conn) {
            self.actor_id.clone()
        } else {
            format!("{}@{}", self.actor_id, self.get_instance(conn).public_domain)
        }
    }

    pub fn to_json(&self, conn: &PgConnection) -> serde_json::Value {
        let mut json = serde_json::to_value(self).unwrap();
        json["fqn"] = json!(self.get_fqn(conn));
        json
    }
}

impl IntoId for Blog {
    fn into_id(self) -> Id {
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

    fn is_local(&self) -> bool {
        self.instance_id == 0
    }
}

impl sign::Signer for Blog {
    fn get_key_id(&self) -> String {
        format!("{}#main-key", self.ap_url)
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
