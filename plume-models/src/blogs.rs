use crate::{
    ap_url, db_conn::DbConn, instance::*, medias::Media, posts::Post, safe_string::SafeString,
    schema::blogs, users::User, Connection, Error, PlumeRocket, Result, CONFIG, ITEMS_PER_PAGE,
};
use activitypub::{
    actor::Group,
    collection::{OrderedCollection, OrderedCollectionPage},
    object::Image,
    CustomObject,
};
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SaveChangesDsl};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign::{Signer, Verifier},
};
use plume_common::activity_pub::{
    inbox::{AsActor, FromId},
    sign, ActivityStream, ApSignature, Id, IntoId, PublicKey, Source,
};
use url::Url;
use webfinger::*;

pub type CustomGroup = CustomObject<ApSignature, Group>;

#[derive(Queryable, Identifiable, Clone, AsChangeset)]
#[changeset_options(treat_none_as_null = "true")]
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
    pub fqn: String,
    pub summary_html: SafeString,
    pub icon_id: Option<i32>,
    pub banner_id: Option<i32>,
    pub theme: Option<String>,
}

#[derive(Default, Insertable)]
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
    pub summary_html: SafeString,
    pub icon_id: Option<i32>,
    pub banner_id: Option<i32>,
    pub theme: Option<String>,
}

const BLOG_PREFIX: &str = "~";

impl Blog {
    insert!(blogs, NewBlog, |inserted, conn| {
        let instance = inserted.get_instance(conn)?;
        if inserted.outbox_url.is_empty() {
            inserted.outbox_url = instance.compute_box(BLOG_PREFIX, &inserted.actor_id, "outbox");
        }

        if inserted.inbox_url.is_empty() {
            inserted.inbox_url = instance.compute_box(BLOG_PREFIX, &inserted.actor_id, "inbox");
        }

        if inserted.ap_url.is_empty() {
            inserted.ap_url = instance.compute_box(BLOG_PREFIX, &inserted.actor_id, "");
        }

        if inserted.fqn.is_empty() {
            if instance.local {
                inserted.fqn = inserted.actor_id.clone();
            } else {
                inserted.fqn = format!("{}@{}", inserted.actor_id, instance.public_domain);
            }
        }

        inserted.save_changes(conn).map_err(Error::from)
    });
    get!(blogs);
    find_by!(blogs, find_by_ap_url, ap_url as &str);
    find_by!(blogs, find_by_name, actor_id as &str, instance_id as i32);

    pub fn get_instance(&self, conn: &Connection) -> Result<Instance> {
        Instance::get(conn, self.instance_id)
    }

    pub fn list_authors(&self, conn: &Connection) -> Result<Vec<User>> {
        use crate::schema::blog_authors;
        use crate::schema::users;
        let authors_ids = blog_authors::table
            .filter(blog_authors::blog_id.eq(self.id))
            .select(blog_authors::author_id);
        users::table
            .filter(users::id.eq_any(authors_ids))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn count_authors(&self, conn: &Connection) -> Result<i64> {
        use crate::schema::blog_authors;
        blog_authors::table
            .filter(blog_authors::blog_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn find_for_author(conn: &Connection, author: &User) -> Result<Vec<Blog>> {
        use crate::schema::blog_authors;
        let author_ids = blog_authors::table
            .filter(blog_authors::author_id.eq(author.id))
            .select(blog_authors::blog_id);
        blogs::table
            .filter(blogs::id.eq_any(author_ids))
            .load::<Blog>(conn)
            .map_err(Error::from)
    }

    pub fn find_by_fqn(conn: &DbConn, fqn: &str) -> Result<Blog> {
        let from_db = blogs::table
            .filter(blogs::fqn.eq(fqn))
            .first(&**conn)
            .optional()?;
        if let Some(from_db) = from_db {
            Ok(from_db)
        } else {
            Blog::fetch_from_webfinger(conn, fqn)
        }
    }

    fn fetch_from_webfinger(conn: &DbConn, acct: &str) -> Result<Blog> {
        resolve_with_prefix(Prefix::Group, acct.to_owned(), true)?
            .links
            .into_iter()
            .find(|l| l.mime_type == Some(String::from("application/activity+json")))
            .ok_or(Error::Webfinger)
            .and_then(|l| Blog::from_id(conn, &l.href?, None, CONFIG.proxy()).map_err(|(_, e)| e))
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<CustomGroup> {
        let mut blog = Group::default();
        blog.ap_actor_props
            .set_preferred_username_string(self.actor_id.clone())?;
        blog.object_props.set_name_string(self.title.clone())?;
        blog.ap_actor_props
            .set_outbox_string(self.outbox_url.clone())?;
        blog.ap_actor_props
            .set_inbox_string(self.inbox_url.clone())?;
        blog.object_props
            .set_summary_string(self.summary_html.to_string())?;
        blog.ap_object_props.set_source_object(Source {
            content: self.summary.clone(),
            media_type: String::from("text/markdown"),
        })?;

        let mut icon = Image::default();
        icon.object_props.set_url_string(
            self.icon_id
                .and_then(|id| Media::get(conn, id).and_then(|m| m.url()).ok())
                .unwrap_or_default(),
        )?;
        icon.object_props.set_attributed_to_link(
            self.icon_id
                .and_then(|id| {
                    Media::get(conn, id)
                        .and_then(|m| Ok(User::get(conn, m.owner_id)?.into_id()))
                        .ok()
                })
                .unwrap_or_else(|| Id::new(String::new())),
        )?;
        blog.object_props.set_icon_object(icon)?;

        let mut banner = Image::default();
        banner.object_props.set_url_string(
            self.banner_id
                .and_then(|id| Media::get(conn, id).and_then(|m| m.url()).ok())
                .unwrap_or_default(),
        )?;
        banner.object_props.set_attributed_to_link(
            self.banner_id
                .and_then(|id| {
                    Media::get(conn, id)
                        .and_then(|m| Ok(User::get(conn, m.owner_id)?.into_id()))
                        .ok()
                })
                .unwrap_or_else(|| Id::new(String::new())),
        )?;
        blog.object_props.set_image_object(banner)?;

        blog.object_props.set_id_string(self.ap_url.clone())?;

        let mut public_key = PublicKey::default();
        public_key.set_id_string(format!("{}#main-key", self.ap_url))?;
        public_key.set_owner_string(self.ap_url.clone())?;
        public_key.set_public_key_pem_string(self.public_key.clone())?;
        let mut ap_signature = ApSignature::default();
        ap_signature.set_public_key_publickey(public_key)?;

        Ok(CustomGroup::new(blog, ap_signature))
    }

    pub fn outbox(&self, conn: &Connection) -> Result<ActivityStream<OrderedCollection>> {
        let mut coll = OrderedCollection::default();
        coll.collection_props.items = serde_json::to_value(self.get_activities(conn))?;
        coll.collection_props
            .set_total_items_u64(self.get_activities(conn).len() as u64)?;
        coll.collection_props
            .set_first_link(Id::new(ap_url(&format!("{}?page=1", &self.outbox_url))))?;
        coll.collection_props
            .set_last_link(Id::new(ap_url(&format!(
                "{}?page={}",
                &self.outbox_url,
                (self.get_activities(conn).len() as u64 + ITEMS_PER_PAGE as u64 - 1) as u64
                    / ITEMS_PER_PAGE as u64
            ))))?;
        Ok(ActivityStream::new(coll))
    }
    pub fn outbox_page(
        &self,
        conn: &Connection,
        (min, max): (i32, i32),
    ) -> Result<ActivityStream<OrderedCollectionPage>> {
        let mut coll = OrderedCollectionPage::default();
        let acts = self.get_activity_page(&conn, (min, max));
        //This still doesn't do anything because the outbox
        //doesn't do anything yet
        coll.collection_page_props.set_next_link(Id::new(&format!(
            "{}?page={}",
            &self.outbox_url,
            min / ITEMS_PER_PAGE + 1
        )))?;
        coll.collection_page_props.set_prev_link(Id::new(&format!(
            "{}?page={}",
            &self.outbox_url,
            min / ITEMS_PER_PAGE - 1
        )))?;
        coll.collection_props.items = serde_json::to_value(acts)?;
        Ok(ActivityStream::new(coll))
    }
    fn get_activities(&self, _conn: &Connection) -> Vec<serde_json::Value> {
        vec![]
    }
    fn get_activity_page(
        &self,
        _conn: &Connection,
        (_min, _max): (i32, i32),
    ) -> Vec<serde_json::Value> {
        vec![]
    }

    pub fn get_keypair(&self) -> Result<PKey<Private>> {
        PKey::from_rsa(Rsa::private_key_from_pem(
            self.private_key.clone()?.as_ref(),
        )?)
        .map_err(Error::from)
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

    pub fn icon_url(&self, conn: &Connection) -> String {
        self.icon_id
            .and_then(|id| Media::get(conn, id).and_then(|m| m.url()).ok())
            .unwrap_or_else(|| "/static/images/default-avatar.png".to_string())
    }

    pub fn banner_url(&self, conn: &Connection) -> Option<String> {
        self.banner_id
            .and_then(|i| Media::get(conn, i).ok())
            .and_then(|c| c.url().ok())
    }

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        for post in Post::get_for_blog(conn, &self)? {
            post.delete(conn)?;
        }
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }
}

impl IntoId for Blog {
    fn into_id(self) -> Id {
        Id::new(self.ap_url)
    }
}

impl FromId<DbConn> for Blog {
    type Error = Error;
    type Object = CustomGroup;

    fn from_db(conn: &DbConn, id: &str) -> Result<Self> {
        Self::find_by_ap_url(&conn, id)
    }

    fn from_activity(conn: &DbConn, acct: CustomGroup) -> Result<Self> {
        let url = Url::parse(&acct.object.object_props.id_string()?)?;
        let inst = url.host_str()?;
        let instance = Instance::find_by_domain(conn, inst).or_else(|_| {
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
        })?;
        let icon_id = acct
            .object
            .object_props
            .icon_image()
            .ok()
            .and_then(|icon| {
                let owner = icon.object_props.attributed_to_link::<Id>().ok()?;
                Media::save_remote(
                    conn,
                    icon.object_props.url_string().ok()?,
                    &User::from_id(conn, &owner, None, CONFIG.proxy()).ok()?,
                )
                .ok()
            })
            .map(|m| m.id);

        let banner_id = acct
            .object
            .object_props
            .image_image()
            .ok()
            .and_then(|banner| {
                let owner = banner.object_props.attributed_to_link::<Id>().ok()?;
                Media::save_remote(
                    conn,
                    banner.object_props.url_string().ok()?,
                    &User::from_id(conn, &owner, None, CONFIG.proxy()).ok()?,
                )
                .ok()
            })
            .map(|m| m.id);

        let name = acct.object.ap_actor_props.preferred_username_string()?;
        if name.contains(&['<', '>', '&', '@', '\'', '"', ' ', '\t'][..]) {
            return Err(Error::InvalidValue);
        }

        Blog::insert(
            conn,
            NewBlog {
                actor_id: name.clone(),
                title: acct.object.object_props.name_string().unwrap_or(name),
                outbox_url: acct.object.ap_actor_props.outbox_string()?,
                inbox_url: acct.object.ap_actor_props.inbox_string()?,
                summary: acct
                    .object
                    .ap_object_props
                    .source_object::<Source>()
                    .map(|s| s.content)
                    .unwrap_or_default(),
                instance_id: instance.id,
                ap_url: acct.object.object_props.id_string()?,
                public_key: acct
                    .custom_props
                    .public_key_publickey()?
                    .public_key_pem_string()?,
                private_key: None,
                banner_id,
                icon_id,
                summary_html: SafeString::new(
                    &acct
                        .object
                        .object_props
                        .summary_string()
                        .unwrap_or_default(),
                ),
                theme: None,
            },
        )
    }
}

impl AsActor<&PlumeRocket> for Blog {
    fn get_inbox_url(&self) -> String {
        self.inbox_url.clone()
    }

    fn get_shared_inbox_url(&self) -> Option<String> {
        None
    }

    fn is_local(&self) -> bool {
        Instance::get_local()
            .map(|i| self.instance_id == i.id)
            .unwrap_or(false)
    }
}

impl sign::Signer for Blog {
    type Error = Error;

    fn get_key_id(&self) -> String {
        format!("{}#main-key", self.ap_url)
    }

    fn sign(&self, to_sign: &str) -> Result<Vec<u8>> {
        let key = self.get_keypair()?;
        let mut signer = Signer::new(MessageDigest::sha256(), &key)?;
        signer.update(to_sign.as_bytes())?;
        signer.sign_to_vec().map_err(Error::from)
    }

    fn verify(&self, data: &str, signature: &[u8]) -> Result<bool> {
        let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref())?)?;
        let mut verifier = Verifier::new(MessageDigest::sha256(), &key)?;
        verifier.update(data.as_bytes())?;
        verifier.verify(&signature).map_err(Error::from)
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
            instance_id,
            public_key: String::from_utf8(pub_key).or(Err(Error::Signature))?,
            private_key: Some(String::from_utf8(priv_key).or(Err(Error::Signature))?),
            ..NewBlog::default()
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        blog_authors::*, instance::tests as instance_tests, medias::NewMedia, tests::db,
        users::tests as usersTests, Connection as Conn,
    };
    use diesel::Connection;

    pub(crate) fn fill_database(conn: &Conn) -> (Vec<User>, Vec<Blog>) {
        instance_tests::fill_database(conn);
        let users = usersTests::fill_database(conn);
        let blog1 = Blog::insert(
            conn,
            NewBlog::new_local(
                "BlogName".to_owned(),
                "Blog name".to_owned(),
                "This is a small blog".to_owned(),
                Instance::get_local().unwrap().id,
            )
            .unwrap(),
        )
        .unwrap();
        let blog2 = Blog::insert(
            conn,
            NewBlog::new_local(
                "MyBlog".to_owned(),
                "My blog".to_owned(),
                "Welcome to my blog".to_owned(),
                Instance::get_local().unwrap().id,
            )
            .unwrap(),
        )
        .unwrap();
        let blog3 = Blog::insert(
            conn,
            NewBlog::new_local(
                "WhyILikePlume".to_owned(),
                "Why I like Plume".to_owned(),
                "In this blog I will explay you why I like Plume so much".to_owned(),
                Instance::get_local().unwrap().id,
            )
            .unwrap(),
        )
        .unwrap();

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog1.id,
                author_id: users[0].id,
                is_owner: true,
            },
        )
        .unwrap();

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog1.id,
                author_id: users[1].id,
                is_owner: false,
            },
        )
        .unwrap();

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog2.id,
                author_id: users[1].id,
                is_owner: true,
            },
        )
        .unwrap();

        BlogAuthor::insert(
            conn,
            NewBlogAuthor {
                blog_id: blog3.id,
                author_id: users[2].id,
                is_owner: true,
            },
        )
        .unwrap();
        (users, vec![blog1, blog2, blog3])
    }

    #[test]
    fn get_instance() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(&conn);

            let blog = Blog::insert(
                &conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(
                blog.get_instance(&conn).unwrap().id,
                Instance::get_local().unwrap().id
            );
            // TODO add tests for remote instance
            Ok(())
        })
    }

    #[test]
    fn authors() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (user, _) = fill_database(&conn);

            let b1 = Blog::insert(
                &conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();
            let b2 = Blog::insert(
                &conn,
                NewBlog::new_local(
                    "Blog".to_owned(),
                    "Blog".to_owned(),
                    "I've named my blog Blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();
            let blog = vec![b1, b2];

            BlogAuthor::insert(
                &conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                &conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[1].id,
                    is_owner: false,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                &conn,
                NewBlogAuthor {
                    blog_id: blog[1].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            assert!(blog[0]
                .list_authors(&conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[0].id));
            assert!(blog[0]
                .list_authors(&conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[1].id));
            assert!(blog[1]
                .list_authors(&conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[0].id));
            assert!(!blog[1]
                .list_authors(&conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[1].id));

            assert!(Blog::find_for_author(&conn, &user[0])
                .unwrap()
                .iter()
                .any(|b| b.id == blog[0].id));
            assert!(Blog::find_for_author(&conn, &user[1])
                .unwrap()
                .iter()
                .any(|b| b.id == blog[0].id));
            assert!(Blog::find_for_author(&conn, &user[0])
                .unwrap()
                .iter()
                .any(|b| b.id == blog[1].id));
            assert!(!Blog::find_for_author(&conn, &user[1])
                .unwrap()
                .iter()
                .any(|b| b.id == blog[1].id));
            Ok(())
        })
    }

    #[test]
    fn find_local() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(&conn);

            let blog = Blog::insert(
                &conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(Blog::find_by_fqn(&conn, "SomeName").unwrap().id, blog.id);
            Ok(())
        })
    }

    #[test]
    fn get_fqn() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(&conn);

            let blog = Blog::insert(
                &conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(blog.fqn, "SomeName");
            Ok(())
        })
    }

    #[test]
    fn delete() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, blogs) = fill_database(&conn);

            blogs[0].delete(&conn).unwrap();
            assert!(Blog::get(&conn, blogs[0].id).is_err());
            Ok(())
        })
    }

    #[test]
    fn delete_via_user() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (user, _) = fill_database(&conn);

            let b1 = Blog::insert(
                &conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();
            let b2 = Blog::insert(
                &conn,
                NewBlog::new_local(
                    "Blog".to_owned(),
                    "Blog".to_owned(),
                    "I've named my blog Blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();
            let blog = vec![b1, b2];

            BlogAuthor::insert(
                &conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                &conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[1].id,
                    is_owner: false,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                &conn,
                NewBlogAuthor {
                    blog_id: blog[1].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            user[0].delete(&conn).unwrap();
            assert!(Blog::get(&conn, blog[0].id).is_ok());
            assert!(Blog::get(&conn, blog[1].id).is_err());
            user[1].delete(&conn).unwrap();
            assert!(Blog::get(&conn, blog[0].id).is_err());
            Ok(())
        })
    }

    #[test]
    fn self_federation() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, mut blogs) = fill_database(&conn);
            blogs[0].icon_id = Some(
                Media::insert(
                    &conn,
                    NewMedia {
                        file_path: "aaa.png".into(),
                        alt_text: String::new(),
                        is_remote: false,
                        remote_url: None,
                        sensitive: false,
                        content_warning: None,
                        owner_id: users[0].id,
                    },
                )
                .unwrap()
                .id,
            );
            blogs[0].banner_id = Some(
                Media::insert(
                    &conn,
                    NewMedia {
                        file_path: "bbb.png".into(),
                        alt_text: String::new(),
                        is_remote: false,
                        remote_url: None,
                        sensitive: false,
                        content_warning: None,
                        owner_id: users[0].id,
                    },
                )
                .unwrap()
                .id,
            );
            let _: Blog = blogs[0].save_changes(&**conn).unwrap();

            let ap_repr = blogs[0].to_activity(&conn).unwrap();
            blogs[0].delete(&conn).unwrap();
            let blog = Blog::from_activity(&conn, ap_repr).unwrap();

            assert_eq!(blog.actor_id, blogs[0].actor_id);
            assert_eq!(blog.title, blogs[0].title);
            assert_eq!(blog.summary, blogs[0].summary);
            assert_eq!(blog.outbox_url, blogs[0].outbox_url);
            assert_eq!(blog.inbox_url, blogs[0].inbox_url);
            assert_eq!(blog.instance_id, blogs[0].instance_id);
            assert_eq!(blog.ap_url, blogs[0].ap_url);
            assert_eq!(blog.public_key, blogs[0].public_key);
            assert_eq!(blog.fqn, blogs[0].fqn);
            assert_eq!(blog.summary_html, blogs[0].summary_html);
            assert_eq!(blog.icon_url(&conn), blogs[0].icon_url(&conn));
            assert_eq!(blog.banner_url(&conn), blogs[0].banner_url(&conn));

            Ok(())
        })
    }
}
