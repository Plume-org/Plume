use activitypub::{Actor, Object, CustomObject, actor::Group, collection::OrderedCollection};
use chrono::NaiveDateTime;
use reqwest::{Client,
    header::{ACCEPT, HeaderValue}
};
use serde_json;
use url::Url;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign::{Signer,Verifier}
};
use webfinger::*;

use {BASE_URL, USE_HTTPS, Connection};
use plume_common::activity_pub::{
    ap_accept_header, ApSignature, ActivityStream, Id, IntoId, PublicKey,
    inbox::{Deletable, WithInbox},
    sign
};
use safe_string::SafeString;
use instance::*;
use posts::Post;
use schema::blogs;
use users::User;

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

    pub fn get_instance(&self, conn: &Connection) -> Instance {
        Instance::get(conn, self.instance_id).expect("Blog::get_instance: instance not found error")
    }

    pub fn list_authors(&self, conn: &Connection) -> Vec<User> {
        use schema::blog_authors;
        use schema::users;
        let authors_ids = blog_authors::table.filter(blog_authors::blog_id.eq(self.id)).select(blog_authors::author_id);
        users::table.filter(users::id.eq_any(authors_ids))
            .load::<User>(conn)
            .expect("Blog::list_authors: author loading error")
    }

    pub fn find_for_author(conn: &Connection, author: &User) -> Vec<Blog> {
        use schema::blog_authors;
        let author_ids = blog_authors::table.filter(blog_authors::author_id.eq(author.id)).select(blog_authors::blog_id);
        blogs::table.filter(blogs::id.eq_any(author_ids))
            .load::<Blog>(conn)
            .expect("Blog::find_for_author: blog loading error")
    }

    pub fn find_local(conn: &Connection, name: String) -> Option<Blog> {
        Blog::find_by_name(conn, name, Instance::local_id(conn))
    }

    pub fn find_by_fqn(conn: &Connection, fqn: String) -> Option<Blog> {
        if fqn.contains("@") { // remote blog
            match Instance::find_by_domain(conn, String::from(fqn.split("@").last().expect("Blog::find_by_fqn: unreachable"))) {
                Some(instance) => {
                    match Blog::find_by_name(conn, String::from(fqn.split("@").nth(0).expect("Blog::find_by_fqn: unreachable")), instance.id) {
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

    fn fetch_from_webfinger(conn: &Connection, acct: String) -> Option<Blog> {
        match resolve(acct.clone(), *USE_HTTPS) {
            Ok(wf) => wf.links.into_iter().find(|l| l.mime_type == Some(String::from("application/activity+json"))).and_then(|l| Blog::fetch_from_url(conn, l.href.expect("Blog::fetch_from_webfinger: href not found error"))),
            Err(details) => {
                println!("{:?}", details);
                None
            }
        }
    }

    fn fetch_from_url(conn: &Connection, url: String) -> Option<Blog> {
        let req = Client::new()
            .get(&url[..])
            .header(ACCEPT, HeaderValue::from_str(&ap_accept_header().into_iter().collect::<Vec<_>>().join(", ")).expect("Blog::fetch_from_url: accept_header generation error"))
            .send();
        match req {
            Ok(mut res) => {
                let text = &res.text().expect("Blog::fetch_from_url: body reading error");
                let ap_sign: ApSignature = serde_json::from_str(text).expect("Blog::fetch_from_url: body parsing error");
                let mut json: CustomGroup = serde_json::from_str(text).expect("Blog::fetch_from_url: body parsing error");
                json.custom_props = ap_sign; // without this workaround, publicKey is not correctly deserialized
                Some(Blog::from_activity(conn, json, Url::parse(url.as_ref()).expect("Blog::fetch_from_url: url parsing error").host_str().expect("Blog::fetch_from_url: host extraction error").to_string()))
            },
            Err(_) => None
        }
    }

    fn from_activity(conn: &Connection, acct: CustomGroup, inst: String) -> Blog {
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

    pub fn into_activity(&self, _conn: &Connection) -> CustomGroup {
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

    pub fn update_boxes(&self, conn: &Connection) {
        let instance = self.get_instance(conn);
        if self.outbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::outbox_url.eq(instance.compute_box(BLOG_PREFIX, self.actor_id.clone(), "outbox")))
                .execute(conn).expect("Blog::update_boxes: outbox update error");
        }

        if self.inbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::inbox_url.eq(instance.compute_box(BLOG_PREFIX, self.actor_id.clone(), "inbox")))
                .execute(conn).expect("Blog::update_boxes: inbox update error");
        }

        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(blogs::ap_url.eq(instance.compute_box(BLOG_PREFIX, self.actor_id.clone(), "")))
                .execute(conn).expect("Blog::update_boxes: ap_url update error");
        }
    }

    pub fn outbox(&self, conn: &Connection) -> ActivityStream<OrderedCollection> {
        let mut coll = OrderedCollection::default();
        coll.collection_props.items = serde_json::to_value(self.get_activities(conn)).expect("Blog::outbox: activity serialization error");
        coll.collection_props.set_total_items_u64(self.get_activities(conn).len() as u64).expect("Blog::outbox: count serialization error");
        ActivityStream::new(coll)
    }

    fn get_activities(&self, _conn: &Connection) -> Vec<serde_json::Value> {
        vec![]
    }

    pub fn get_keypair(&self) -> PKey<Private> {
        PKey::from_rsa(Rsa::private_key_from_pem(self.private_key.clone().expect("Blog::get_keypair: private key not found error").as_ref())
                       .expect("Blog::get_keypair: pem parsing error"))
            .expect("Blog::get_keypair: private key deserialization error")
    }

    pub fn webfinger(&self, conn: &Connection) -> Webfinger {
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

    pub fn from_url(conn: &Connection, url: String) -> Option<Blog> {
        Blog::find_by_ap_url(conn, url.clone()).or_else(|| {
            // The requested blog was not in the DB
            // We try to fetch it if it is remote
            if Url::parse(url.as_ref()).expect("Blog::from_url: ap_url parsing error").host_str().expect("Blog::from_url: host extraction error") != BASE_URL.as_str() {
                Blog::fetch_from_url(conn, url)
            } else {
                None
            }
        })
    }

    pub fn get_fqn(&self, conn: &Connection) -> String {
        if self.instance_id == Instance::local_id(conn) {
            self.actor_id.clone()
        } else {
            format!("{}@{}", self.actor_id, self.get_instance(conn).public_domain)
        }
    }

    pub fn to_json(&self, conn: &Connection) -> serde_json::Value {
        let mut json = serde_json::to_value(self).expect("Blog::to_json: serialization error");
        json["fqn"] = json!(self.get_fqn(conn));
        json
    }

    pub fn delete(&self, conn: &Connection) {
        for post in Post::get_for_blog(conn, &self) {
           post.delete(conn);
        }
        diesel::delete(self).execute(conn).expect("Blog::delete: blog deletion error");
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
        let mut signer = Signer::new(MessageDigest::sha256(), &key).expect("Blog::sign: initialization error");
        signer.update(to_sign.as_bytes()).expect("Blog::sign: content insertion error");
        signer.sign_to_vec().expect("Blog::sign: finalization error")
    }

    fn verify(&self, data: String, signature: Vec<u8>) -> bool {
       let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref()).expect("Blog::verify: pem parsing error"))
           .expect("Blog::verify: deserialization error");
       let mut verifier = Verifier::new(MessageDigest::sha256(), &key).expect("Blog::verify: initialization error");
        verifier.update(data.as_bytes()).expect("Blog::verify: content insertion error");
        verifier.verify(&signature).expect("Blog::verify: finalization error")
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
            public_key: String::from_utf8(pub_key).expect("NewBlog::new_local: public key error"),
            private_key: Some(String::from_utf8(priv_key).expect("NewBlog::new_local: private key error"))
        }
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use diesel::Connection;
    use Connection as Conn;
    use tests::db;
    use super::*;
    use blog_authors::*;
    use instance::tests as instanceTests;
    use users::tests as usersTests;

    pub(crate) fn fill_database(conn: &Conn) -> Vec<Blog> {
        instanceTests::fill_database(conn);
        let users = usersTests::fill_database(conn);
        let blogs = vec![
            NewBlog::new_local(
                "BlogName".to_owned(),
                "Blog name".to_owned(),
                "This is a small blog".to_owned(),
                Instance::local_id(conn)
                ),
            NewBlog::new_local(
                "MyBlog".to_owned(),
                "My blog".to_owned(),
                "Welcome to my blog".to_owned(),
                Instance::local_id(conn)
                ),
            NewBlog::new_local(
                "WhyILikePlume".to_owned(),
                "Why I like Plume".to_owned(),
                "In this blog I will explay you why I like Plume so much".to_owned(),
                Instance::local_id(conn)
                )
        ].into_iter().map(|nb| Blog::insert(conn, nb)).collect::<Vec<_>>();

        BlogAuthor::insert(conn, NewBlogAuthor {
            blog_id: blogs[0].id,
            author_id: users[0].id,
            is_owner: true,
        });

        BlogAuthor::insert(conn, NewBlogAuthor {
            blog_id: blogs[0].id,
            author_id: users[1].id,
            is_owner: false,
        });

        BlogAuthor::insert(conn, NewBlogAuthor {
            blog_id: blogs[1].id,
            author_id: users[1].id,
            is_owner: true,
        });

        BlogAuthor::insert(conn, NewBlogAuthor {
            blog_id: blogs[2].id,
            author_id: users[2].id,
            is_owner: true,
        });
        blogs
    }

    #[test]
    fn get_instance() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let blog = Blog::insert(conn, NewBlog::new_local(
                "SomeName".to_owned(),
                "Some name".to_owned(),
                "This is some blog".to_owned(),
                Instance::local_id(conn)
                ));

            assert_eq!(blog.get_instance(conn).id, Instance::local_id(conn));
            // TODO add tests for remote instance

            Ok(())
        });
    }

    #[test]
    fn authors() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let user = usersTests::fill_database(conn);
            fill_database(conn);

            let blog = vec![
                Blog::insert(conn, NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::local_id(conn)
                )),
                Blog::insert(conn, NewBlog::new_local(
                    "Blog".to_owned(),
                    "Blog".to_owned(),
                    "I've named my blog Blog".to_owned(),
                    Instance::local_id(conn)
                ))
            ];

            BlogAuthor::insert(conn, NewBlogAuthor {
                blog_id: blog[0].id,
                author_id: user[0].id,
                is_owner: true,
            });

            BlogAuthor::insert(conn, NewBlogAuthor {
                blog_id: blog[0].id,
                author_id: user[1].id,
                is_owner: false,
            });

            BlogAuthor::insert(conn, NewBlogAuthor {
                blog_id: blog[1].id,
                author_id: user[0].id,
                is_owner: true,
            });

            assert!(blog[0].list_authors(conn).iter().any(|a| a.id==user[0].id));
            assert!(blog[0].list_authors(conn).iter().any(|a| a.id==user[1].id));
            assert!(blog[1].list_authors(conn).iter().any(|a| a.id==user[0].id));
            assert!(!blog[1].list_authors(conn).iter().any(|a| a.id==user[1].id));

            assert!(Blog::find_for_author(conn, &user[0]).iter().any(|b| b.id==blog[0].id));
            assert!(Blog::find_for_author(conn, &user[1]).iter().any(|b| b.id==blog[0].id));
            assert!(Blog::find_for_author(conn, &user[0]).iter().any(|b| b.id==blog[1].id));
            assert!(!Blog::find_for_author(conn, &user[1]).iter().any(|b| b.id==blog[1].id));

            Ok(())
        });
    }

    #[test]
    fn find_local() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let blog = Blog::insert(conn, NewBlog::new_local(
                "SomeName".to_owned(),
                "Some name".to_owned(),
                "This is some blog".to_owned(),
                Instance::local_id(conn)
                ));

            assert_eq!(Blog::find_local(conn, "SomeName".to_owned()).unwrap().id, blog.id);

            Ok(())
        });
    }

    #[test]
    fn get_fqn() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let blog = Blog::insert(conn, NewBlog::new_local(
                "SomeName".to_owned(),
                "Some name".to_owned(),
                "This is some blog".to_owned(),
                Instance::local_id(conn)
                ));

            assert_eq!(blog.get_fqn(conn), "SomeName");

            Ok(())
        });
    }

    #[test]
    fn delete() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let blogs = fill_database(conn);

            blogs[0].delete(conn);
            assert!(Blog::get(conn, blogs[0].id).is_none());

            Ok(())
        });
    }

    #[test]
    fn delete_via_user() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let user = usersTests::fill_database(conn);
            fill_database(conn);

            let blog = vec![
                Blog::insert(conn, NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::local_id(conn)
                )),
                Blog::insert(conn, NewBlog::new_local(
                    "Blog".to_owned(),
                    "Blog".to_owned(),
                    "I've named my blog Blog".to_owned(),
                    Instance::local_id(conn)
                ))
            ];

            BlogAuthor::insert(conn, NewBlogAuthor {
                blog_id: blog[0].id,
                author_id: user[0].id,
                is_owner: true,
            });

            BlogAuthor::insert(conn, NewBlogAuthor {
                blog_id: blog[0].id,
                author_id: user[1].id,
                is_owner: false,
            });

            BlogAuthor::insert(conn, NewBlogAuthor {
                blog_id: blog[1].id,
                author_id: user[0].id,
                is_owner: true,
            });

            user[0].delete(conn);
            assert!(Blog::get(conn, blog[0].id).is_some());
            assert!(Blog::get(conn, blog[1].id).is_none());
            user[1].delete(conn);
            assert!(Blog::get(conn, blog[0].id).is_none());

            Ok(())
        });
    }
}
