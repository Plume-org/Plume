use activitypub::{actor::Group, collection::OrderedCollection, Actor, CustomObject, Object};
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign::{Signer, Verifier},
};
use reqwest::{
    header::{HeaderValue, ACCEPT},
    Client,
};
use serde_json;
use url::Url;
use webfinger::*;

use instance::*;
use plume_common::activity_pub::{
    ap_accept_header,
    inbox::{Deletable, WithInbox},
    sign, ActivityStream, ApSignature, Id, IntoId, PublicKey,
};
use posts::Post;
use safe_string::SafeString;
use schema::blogs;
use search::Searcher;
use users::User;
use {Connection, BASE_URL, USE_HTTPS, Error, Result};

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
    pub public_key: String,
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
    pub public_key: String,
}

const BLOG_PREFIX: &str = "~";

impl Blog {
    insert!(blogs, NewBlog);
    get!(blogs);
    find_by!(blogs, find_by_ap_url, ap_url as &str);
    find_by!(blogs, find_by_name, actor_id as &str, instance_id as i32);

    pub fn get_instance(&self, conn: &Connection) -> Result<Instance> {
        Instance::get(conn, self.instance_id)
    }

    pub fn list_authors(&self, conn: &Connection) -> Result<Vec<User>> {
        use schema::blog_authors;
        use schema::users;
        let authors_ids = blog_authors::table
            .filter(blog_authors::blog_id.eq(self.id))
            .select(blog_authors::author_id);
        users::table
            .filter(users::id.eq_any(authors_ids))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn count_authors(&self, conn: &Connection) -> Result<i64> {
        use schema::blog_authors;
        blog_authors::table
            .filter(blog_authors::blog_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn find_for_author(conn: &Connection, author: &User) -> Result<Vec<Blog>> {
        use schema::blog_authors;
        let author_ids = blog_authors::table
            .filter(blog_authors::author_id.eq(author.id))
            .select(blog_authors::blog_id);
        blogs::table
            .filter(blogs::id.eq_any(author_ids))
            .load::<Blog>(conn)
            .map_err(Error::from)
    }

    pub fn find_local(conn: &Connection, name: &str) -> Result<Blog> {
        Blog::find_by_name(conn, name, Instance::get_local(conn)?.id)
    }

    pub fn find_by_fqn(conn: &Connection, fqn: &str) -> Result<Blog> {
        let mut split_fqn = fqn.split('@');
        let actor = split_fqn.next().ok_or(Error::InvalidValue)?;
        if let Some(domain) = split_fqn.next() { // remote blog
            Instance::find_by_domain(conn, domain)
                .and_then(|instance| Blog::find_by_name(conn, actor, instance.id))
                .or_else(|_| Blog::fetch_from_webfinger(conn, fqn))
        } else { // local blog
            Blog::find_local(conn, actor)
        }
    }

    fn fetch_from_webfinger(conn: &Connection, acct: &str) -> Result<Blog> {
        resolve(acct.to_owned(), *USE_HTTPS)?.links
            .into_iter()
            .find(|l| l.mime_type == Some(String::from("application/activity+json")))
            .ok_or(Error::Webfinger)
            .and_then(|l| {
                Blog::fetch_from_url(
                    conn,
                    &l.href?
                )
            })
    }

    fn fetch_from_url(conn: &Connection, url: &str) -> Result<Blog> {
        let mut res = Client::new()
            .get(url)
            .header(
                ACCEPT,
                HeaderValue::from_str(
                    &ap_accept_header()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", "),
                )?,
            )
            .send()?;

        let text = &res.text()?;
        let ap_sign: ApSignature =
            serde_json::from_str(text)?;
        let mut json: CustomGroup =
            serde_json::from_str(text)?;
        json.custom_props = ap_sign; // without this workaround, publicKey is not correctly deserialized
        Blog::from_activity(
            conn,
            &json,
            Url::parse(url)?.host_str()?,
        )
    }

    fn from_activity(conn: &Connection, acct: &CustomGroup, inst: &str) -> Result<Blog> {
        let instance = Instance::find_by_domain(conn, inst).or_else(|_|
            Instance::insert(
                conn,
                NewInstance {
                    public_domain: inst.to_owned(),
                    name: inst.to_owned(),
                    local: false,
                    // We don't really care about all the following for remote instances
                    long_description: SafeString::new(""),
                    short_description: SafeString::new(""),
                    default_license: String::new(),
                    open_registrations: true,
                    short_description_html: String::new(),
                    long_description_html: String::new(),
                },
            )
        )?;
        Blog::insert(
            conn,
            NewBlog {
                actor_id: acct
                    .object
                    .ap_actor_props
                    .preferred_username_string()?,
                title: acct
                    .object
                    .object_props
                    .name_string()?,
                outbox_url: acct
                    .object
                    .ap_actor_props
                    .outbox_string()?,
                inbox_url: acct
                    .object
                    .ap_actor_props
                    .inbox_string()?,
                summary: acct
                    .object
                    .object_props
                    .summary_string()?,
                instance_id: instance.id,
                ap_url: acct
                    .object
                    .object_props
                    .id_string()?,
                public_key: acct
                    .custom_props
                    .public_key_publickey()?
                    .public_key_pem_string()?,
                private_key: None,
            },
        )
    }

    pub fn to_activity(&self, _conn: &Connection) -> Result<CustomGroup> {
        let mut blog = Group::default();
        blog.ap_actor_props
            .set_preferred_username_string(self.actor_id.clone())?;
        blog.object_props
            .set_name_string(self.title.clone())?;
        blog.ap_actor_props
            .set_outbox_string(self.outbox_url.clone())?;
        blog.ap_actor_props
            .set_inbox_string(self.inbox_url.clone())?;
        blog.object_props
            .set_summary_string(self.summary.clone())?;
        blog.object_props
            .set_id_string(self.ap_url.clone())?;

        let mut public_key = PublicKey::default();
        public_key
            .set_id_string(format!("{}#main-key", self.ap_url))?;
        public_key
            .set_owner_string(self.ap_url.clone())?;
        public_key
            .set_public_key_pem_string(self.public_key.clone())?;
        let mut ap_signature = ApSignature::default();
        ap_signature
            .set_public_key_publickey(public_key)?;

        Ok(CustomGroup::new(blog, ap_signature))
    }

    pub fn update_boxes(&self, conn: &Connection) -> Result<()> {
        let instance = self.get_instance(conn)?;
        if self.outbox_url.is_empty() {
            diesel::update(self)
                .set(blogs::outbox_url.eq(instance.compute_box(
                    BLOG_PREFIX,
                    &self.actor_id,
                    "outbox",
                )))
                .execute(conn)?;
        }

        if self.inbox_url.is_empty() {
            diesel::update(self)
                .set(blogs::inbox_url.eq(instance.compute_box(
                    BLOG_PREFIX,
                    &self.actor_id,
                    "inbox",
                )))
                .execute(conn)?;
        }

        if self.ap_url.is_empty() {
            diesel::update(self)
                .set(blogs::ap_url.eq(instance.compute_box(BLOG_PREFIX, &self.actor_id, "")))
                .execute(conn)?;
        }
        Ok(())
    }

    pub fn outbox(&self, conn: &Connection) -> Result<ActivityStream<OrderedCollection>> {
        let mut coll = OrderedCollection::default();
        coll.collection_props.items = serde_json::to_value(self.get_activities(conn)?)?;
        coll.collection_props
            .set_total_items_u64(self.get_activities(conn)?.len() as u64)?;
        Ok(ActivityStream::new(coll))
    }

    fn get_activities(&self, _conn: &Connection) -> Result<Vec<serde_json::Value>> {
        Ok(vec![])
    }

    pub fn get_keypair(&self) -> Result<PKey<Private>> {
        PKey::from_rsa(
            Rsa::private_key_from_pem(
                self.private_key
                    .clone()?
                    .as_ref(),
            )?,
        ).map_err(Error::from)
    }

    pub fn webfinger(&self, conn: &Connection) -> Result<Webfinger> {
        Ok(Webfinger {
            subject: format!(
                "acct:{}@{}",
                self.actor_id,
                self.get_instance(conn)?.public_domain
            ),
            aliases: vec![self.ap_url.clone()],
            links: vec![
                Link {
                    rel: String::from("http://webfinger.net/rel/profile-page"),
                    mime_type: None,
                    href: Some(self.ap_url.clone()),
                    template: None,
                },
                Link {
                    rel: String::from("http://schemas.google.com/g/2010#updates-from"),
                    mime_type: Some(String::from("application/atom+xml")),
                    href: Some(self.get_instance(conn)?.compute_box(
                        BLOG_PREFIX,
                        &self.actor_id,
                        "feed.atom",
                    )),
                    template: None,
                },
                Link {
                    rel: String::from("self"),
                    mime_type: Some(String::from("application/activity+json")),
                    href: Some(self.ap_url.clone()),
                    template: None,
                },
            ],
        })
    }

    pub fn from_url(conn: &Connection, url: &str) -> Result<Blog> {
        Blog::find_by_ap_url(conn, url).or_else(|_| {
            // The requested blog was not in the DB
            // We try to fetch it if it is remote
            if Url::parse(url)?.host_str()? != BASE_URL.as_str() {
                Blog::fetch_from_url(conn, url)
            } else {
                Err(Error::NotFound)
            }
        })
    }

    pub fn get_fqn(&self, conn: &Connection) -> String {
        if self.instance_id == Instance::get_local(conn).ok().expect("Blog::get_fqn: local instance error").id {
            self.actor_id.clone()
        } else {
            format!(
                "{}@{}",
                self.actor_id,
                self.get_instance(conn).ok().expect("Blog::get_fqn: instance error").public_domain
            )
        }
    }

    pub fn delete(&self, conn: &Connection, searcher: &Searcher) -> Result<usize> {
        for post in Post::get_for_blog(conn, &self)? {
            post.delete(&(conn, searcher))?;
        }
        diesel::delete(self).execute(conn).map_err(Error::from)
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
    type Error = Error;

    fn get_key_id(&self) -> String {
        format!("{}#main-key", self.ap_url)
    }

    fn sign(&self, to_sign: &str) -> Result<Vec<u8>> {
        let key = self.get_keypair()?;
        let mut signer =
            Signer::new(MessageDigest::sha256(), &key)?;
        signer
            .update(to_sign.as_bytes())?;
        signer
            .sign_to_vec()
            .map_err(Error::from)
    }

    fn verify(&self, data: &str, signature: &[u8]) -> Result<bool> {
        let key = PKey::from_rsa(
            Rsa::public_key_from_pem(self.public_key.as_ref())?
        )?;
        let mut verifier = Verifier::new(MessageDigest::sha256(), &key)?;
        verifier
            .update(data.as_bytes())?;
        verifier
            .verify(&signature)
            .map_err(Error::from)
    }
}

impl NewBlog {
    pub fn new_local(
        actor_id: String,
        title: String,
        summary: String,
        instance_id: i32,
    ) -> Result<NewBlog> {
        let (pub_key, priv_key) = sign::gen_keypair();
        Ok(NewBlog {
            actor_id,
            title,
            summary,
            outbox_url: String::from(""),
            inbox_url: String::from(""),
            instance_id,
            ap_url: String::from(""),
            public_key: String::from_utf8(pub_key).or(Err(Error::Signature))?,
            private_key: Some(String::from_utf8(priv_key).or(Err(Error::Signature))?),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use blog_authors::*;
    use diesel::Connection;
    use instance::tests as instance_tests;
    use tests::db;
    use users::tests as usersTests;
    use search::tests::get_searcher;
    use Connection as Conn;

    pub(crate) fn fill_database(conn: &Conn) -> (Vec<User>, Vec<Blog>) {
        instance_tests::fill_database(conn);
        let users = usersTests::fill_database(conn);
        let blog1 = Blog::insert(conn, NewBlog::new_local(
            "BlogName".to_owned(),
            "Blog name".to_owned(),
            "This is a small blog".to_owned(),
            Instance::local_id(conn),
        ));
        blog1.update_boxes(conn);
        let blog2 = Blog::insert(conn, NewBlog::new_local(
                "MyBlog".to_owned(),
                "My blog".to_owned(),
                "Welcome to my blog".to_owned(),
                Instance::local_id(conn),
        ));
        blog2.update_boxes(conn);
        let blog3 = Blog::insert(conn, NewBlog::new_local(
                "WhyILikePlume".to_owned(),
                "Why I like Plume".to_owned(),
                "In this blog I will explay you why I like Plume so much".to_owned(),
                Instance::local_id(conn),
        ));
        blog3.update_boxes(conn);

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog1.id,
                author_id: users[0].id,
                is_owner: true,
            },
        );

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog1.id,
                author_id: users[1].id,
                is_owner: false,
            },
        );

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog2.id,
                author_id: users[1].id,
                is_owner: true,
            },
        );

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog3.id,
                author_id: users[2].id,
                is_owner: true,
            },
        );
        (users, vec![ blog1, blog2, blog3 ])
    }

    #[test]
    fn get_instance() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let blog = Blog::insert(
                conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::local_id(conn),
                ),
            );

            assert_eq!(blog.get_instance(conn).id, Instance::local_id(conn));
            // TODO add tests for remote instance

            Ok(())
        });
    }

    #[test]
    fn authors() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (user, _) = fill_database(conn);

            let b1 = Blog::insert(
                conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::local_id(conn),
                ),
            );
            b1.update_boxes(conn);
            let b2 = Blog::insert(
                conn,
                NewBlog::new_local(
                    "Blog".to_owned(),
                    "Blog".to_owned(),
                    "I've named my blog Blog".to_owned(),
                    Instance::local_id(conn),
                ),
            );
            b2.update_boxes(conn);
            let blog = vec![ b1, b2 ];

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            );

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[1].id,
                    is_owner: false,
                },
            );

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[1].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            );

            assert!(
                blog[0]
                    .list_authors(conn)
                    .iter()
                    .any(|a| a.id == user[0].id)
            );
            assert!(
                blog[0]
                    .list_authors(conn)
                    .iter()
                    .any(|a| a.id == user[1].id)
            );
            assert!(
                blog[1]
                    .list_authors(conn)
                    .iter()
                    .any(|a| a.id == user[0].id)
            );
            assert!(
                !blog[1]
                    .list_authors(conn)
                    .iter()
                    .any(|a| a.id == user[1].id)
            );

            assert!(
                Blog::find_for_author(conn, &user[0])
                    .iter()
                    .any(|b| b.id == blog[0].id)
            );
            assert!(
                Blog::find_for_author(conn, &user[1])
                    .iter()
                    .any(|b| b.id == blog[0].id)
            );
            assert!(
                Blog::find_for_author(conn, &user[0])
                    .iter()
                    .any(|b| b.id == blog[1].id)
            );
            assert!(
                !Blog::find_for_author(conn, &user[1])
                    .iter()
                    .any(|b| b.id == blog[1].id)
            );

            Ok(())
        });
    }

    #[test]
    fn find_local() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let blog = Blog::insert(
                conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::local_id(conn),
                ),
            );

            assert_eq!(
                Blog::find_local(conn, "SomeName").unwrap().id,
                blog.id
            );

            Ok(())
        });
    }

    #[test]
    fn get_fqn() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let blog = Blog::insert(
                conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::local_id(conn),
                ),
            );

            assert_eq!(blog.get_fqn(conn), "SomeName");

            Ok(())
        });
    }

    #[test]
    fn delete() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, blogs) = fill_database(conn);

            blogs[0].delete(conn, &get_searcher());
            assert!(Blog::get(conn, blogs[0].id).is_none());

            Ok(())
        });
    }

    #[test]
    fn delete_via_user() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let searcher = get_searcher();
            let (user, _) = fill_database(conn);

            let b1 = Blog::insert(
                conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::local_id(conn),
                ),
            );
            b1.update_boxes(conn);
            let b2 = Blog::insert(
                conn,
                NewBlog::new_local(
                    "Blog".to_owned(),
                    "Blog".to_owned(),
                    "I've named my blog Blog".to_owned(),
                    Instance::local_id(conn),
                ),
            );
            b2.update_boxes(conn);
            let blog = vec![ b1, b2 ];

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            );

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[1].id,
                    is_owner: false,
                },
            );

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[1].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            );

            user[0].delete(conn, &searcher);
            assert!(Blog::get(conn, blog[0].id).is_some());
            assert!(Blog::get(conn, blog[1].id).is_none());
            user[1].delete(conn, &searcher);
            assert!(Blog::get(conn, blog[0].id).is_none());

            Ok(())
        });
    }
}
