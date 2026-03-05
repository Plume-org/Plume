use crate::{
    instance::*, medias::Media, posts::Post, safe_string::SafeString, schema::blogs, users::User,
    Connection, Error, PlumeRocket, Result, CONFIG, ITEMS_PER_PAGE,
};
use activitystreams::{
    actor::{ApActor, ApActorExt, AsApActor, Group},
    base::AnyBase,
    collection::{OrderedCollection, OrderedCollectionPage},
    iri_string::types::IriString,
    object::{kind::ImageType, ApObject, Image, ObjectExt},
    prelude::*,
};
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SaveChangesDsl};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign::{Signer, Verifier},
};
use plume_common::{
    activity_pub::{
        inbox::{AsActor, FromId},
        sign, ActivityStream, ApSignature, CustomGroup, Id, IntoId, PublicKey, Source,
        SourceProperty, ToAsString, ToAsUri,
    },
    utils::iri_percent_encode_seg,
};
use webfinger::*;

#[derive(Queryable, Identifiable, Insertable, Clone, AsChangeset, Debug)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = blogs)]
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
#[diesel(table_name = blogs)]
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
                inserted.fqn = iri_percent_encode_seg(&inserted.actor_id);
            } else {
                inserted.fqn = format!(
                    "{}@{}",
                    iri_percent_encode_seg(&inserted.actor_id),
                    instance.public_domain
                );
            }
        }

        inserted.save_changes(conn).map_err(Error::from)
    });
    get!(blogs);
    find_by!(blogs, find_by_ap_url, ap_url as &str);
    find_by!(blogs, find_by_name, actor_id as &str, instance_id as i32);

    pub fn slug(title: &str) -> &str {
        title
    }

    pub fn get_instance(&self, conn: &mut Connection) -> Result<Instance> {
        Instance::get(conn, self.instance_id)
    }

    pub fn save(&self, conn: &mut Connection) -> Result<usize> {
        diesel::insert_into(blogs::table) // Insert or update
            .values(&*self)
            .on_conflict(blogs::id)
            .do_update()
            .set(&*self)
            .execute(conn)
            .map_err(Error::from)
    }

    pub fn list_authors(&self, conn: &mut Connection) -> Result<Vec<User>> {
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

    pub fn count_authors(&self, conn: &mut Connection) -> Result<i64> {
        use crate::schema::blog_authors;
        blog_authors::table
            .filter(blog_authors::blog_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn find_for_author(conn: &mut Connection, author: &User) -> Result<Vec<Blog>> {
        use crate::schema::blog_authors;
        let author_ids = blog_authors::table
            .filter(blog_authors::author_id.eq(author.id))
            .select(blog_authors::blog_id);
        blogs::table
            .filter(blogs::id.eq_any(author_ids))
            .load::<Blog>(conn)
            .map_err(Error::from)
    }

    pub async fn find_by_fqn(conn: &mut Connection, fqn: &str) -> Result<Blog> {
        let from_db = blogs::table
            .filter(blogs::fqn.eq(fqn))
            .first(conn)
            .optional()?;
        if let Some(from_db) = from_db {
            Ok(from_db)
        } else {
            Blog::fetch_from_webfinger(conn, fqn).await
        }
    }

    async fn fetch_from_webfinger(conn: &mut Connection, acct: &str) -> Result<Blog> {
        let rwp = resolve_with_prefix(Prefix::Group, acct.to_owned(), true).await?
            .links
            .into_iter()
            .find(|l| l.mime_type == Some(String::from("application/activity+json")))
            .ok_or(Error::Webfinger);

        match rwp {
            Ok(l) => {
                Blog::from_id(
                    conn,
                    &l.href.ok_or(Error::MissingApProperty)?,
                    None,
                    CONFIG.proxy(),
                )
                .await
                .map_err(|(_, e)| e)
            },
            Err(err) => Err(err),
        }
    }

    pub fn to_activity(&self, conn: &mut Connection) -> Result<CustomGroup> {
        let mut blog = ApActor::new(self.inbox_url.parse()?, Group::new());
        blog.set_preferred_username(iri_percent_encode_seg(&self.actor_id));
        blog.set_name(self.title.clone());
        blog.set_outbox(self.outbox_url.parse()?);
        blog.set_summary(self.summary_html.to_string());
        let source = SourceProperty {
            source: Source {
                content: self.summary.clone(),
                media_type: String::from("text/markdown"),
            },
        };

        let mut icon = Image::new();
        let _ = self.icon_id.map(|id| {
            Media::get(conn, id).and_then(|m| {
                let _ = m
                    .url()
                    .and_then(|url| url.parse::<IriString>().map_err(|_| Error::Url))
                    .map(|url| icon.set_url(url));
                icon.set_attributed_to(
                    User::get(conn, m.owner_id)?
                        .into_id()
                        .parse::<IriString>()?,
                );
                Ok(())
            })
        });
        blog.set_icon(icon.into_any_base()?);

        let mut banner = Image::new();
        let _ = self.banner_id.map(|id| {
            Media::get(conn, id).and_then(|m| {
                let _ = m
                    .url()
                    .and_then(|url| url.parse::<IriString>().map_err(|_| Error::Url))
                    .map(|url| banner.set_url(url));
                banner.set_attributed_to(
                    User::get(conn, m.owner_id)?
                        .into_id()
                        .parse::<IriString>()?,
                );
                Ok(())
            })
        });
        blog.set_image(banner.into_any_base()?);

        blog.set_id(self.ap_url.parse()?);

        let pub_key = PublicKey {
            id: format!("{}#main-key", self.ap_url).parse()?,
            owner: self.ap_url.parse()?,
            public_key_pem: self.public_key.clone(),
        };
        let ap_signature = ApSignature {
            public_key: pub_key,
        };

        Ok(CustomGroup::new(blog, ap_signature, source))
    }

    pub fn outbox(&self, conn: &mut Connection) -> Result<ActivityStream<OrderedCollection>> {
        self.outbox_collection(conn).map(ActivityStream::new)
    }
    pub fn outbox_collection(&self, conn: &mut Connection) -> Result<OrderedCollection> {
        let acts = self.get_activities(conn);
        let acts = acts
            .iter()
            .filter_map(|value| AnyBase::from_arbitrary_json(value).ok())
            .collect::<Vec<AnyBase>>();
        let n_acts = acts.len();
        let mut coll = OrderedCollection::new();
        coll.set_many_items(acts);
        coll.set_total_items(n_acts as u64);
        coll.set_first(format!("{}?page=1", &self.outbox_url).parse::<IriString>()?);
        coll.set_last(
            format!(
                "{}?page={}",
                &self.outbox_url,
                (n_acts as u64 + ITEMS_PER_PAGE as u64 - 1) as u64 / ITEMS_PER_PAGE as u64
            )
            .parse::<IriString>()?,
        );
        Ok(coll)
    }
    pub fn outbox_page(
        &self,
        conn: &mut Connection,
        (min, max): (i32, i32),
    ) -> Result<ActivityStream<OrderedCollectionPage>> {
        self.outbox_collection_page(conn, (min, max))
            .map(ActivityStream::new)
    }
    pub fn outbox_collection_page(
        &self,
        conn: &mut Connection,
        (min, max): (i32, i32),
    ) -> Result<OrderedCollectionPage> {
        let mut coll = OrderedCollectionPage::new();
        let acts = self.get_activity_page(conn, (min, max));
        //This still doesn't do anything because the outbox
        //doesn't do anything yet
        coll.set_next(
            format!("{}?page={}", &self.outbox_url, min / ITEMS_PER_PAGE + 1)
                .parse::<IriString>()?,
        );
        coll.set_prev(
            format!("{}?page={}", &self.outbox_url, min / ITEMS_PER_PAGE - 1)
                .parse::<IriString>()?,
        );
        coll.set_many_items(
            acts.iter()
                .filter_map(|value| AnyBase::from_arbitrary_json(value).ok()),
        );
        Ok(coll)
    }
    fn get_activities(&self, _conn: &mut Connection) -> Vec<serde_json::Value> {
        vec![]
    }
    fn get_activity_page(
        &self,
        _conn: &mut Connection,
        (_min, _max): (i32, i32),
    ) -> Vec<serde_json::Value> {
        vec![]
    }

    pub fn get_keypair(&self) -> Result<PKey<Private>> {
        PKey::from_rsa(Rsa::private_key_from_pem(
            self.private_key
                .clone()
                .ok_or(Error::MissingApProperty)?
                .as_ref(),
        )?)
        .map_err(Error::from)
    }

    pub fn webfinger(&self, conn: &mut Connection) -> Result<Webfinger> {
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

    pub fn icon_url(&self, conn: &mut Connection) -> String {
        self.icon_id
            .and_then(|id| Media::get(conn, id).and_then(|m| m.url()).ok())
            .unwrap_or_else(|| "/static/images/default-avatar.png".to_string())
    }

    pub fn banner_url(&self, conn: &mut Connection) -> Option<String> {
        self.banner_id
            .and_then(|i| Media::get(conn, i).ok())
            .and_then(|c| c.url().ok())
    }

    pub fn delete(&self, conn: &mut Connection) -> Result<()> {
        for post in Post::get_for_blog(conn, self)? {
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

impl FromId<Connection> for Blog {
    type Error = Error;
    type Object = CustomGroup;

    fn from_db(conn: &mut Connection, id: &str) -> Result<Self> {
        Self::find_by_ap_url(conn, id)
    }

    async fn from_activity(conn: &mut Connection, acct: CustomGroup) -> Result<Self> {
        let (name, outbox_url, inbox_url) = {
            let actor = acct.ap_actor_ref();
            let name = actor
                .preferred_username()
                .ok_or(Error::MissingApProperty)?
                .to_string();
            if name.contains(&['<', '>', '&', '@', '\'', '"', ' ', '\t'][..]) {
                tracing::error!("preferredUsername includes invalid character(s): {}", &name);
                return Err(Error::InvalidValue);
            }
            (
                name,
                actor.outbox()?.ok_or(Error::MissingApProperty)?.to_string(),
                actor.inbox()?.to_string(),
            )
        };

        let mut new_blog = NewBlog {
            actor_id: name.to_string(),
            outbox_url,
            inbox_url,
            public_key: acct.ext_one.public_key.public_key_pem.to_string(),
            private_key: None,
            theme: None,
            ..NewBlog::default()
        };

        let object = ApObject::new(acct.inner);
        new_blog.title = object
            .name()
            .and_then(|name| name.to_as_string())
            .unwrap_or(name);
        new_blog.summary_html = SafeString::new(
            &object
                .summary()
                .and_then(|summary| summary.to_as_string())
                .unwrap_or_default(),
        );

        let icon_id = match object.icon()
                .and_then(|icons| icons.iter().next())
                .and_then(|icon|  icon.to_owned().extend::<Image, ImageType>().ok().flatten() )
                .and_then(|icon|  {
                    let i = icon.url().and_then(|b| b.to_as_uri());
                    let o = icon.attributed_to().and_then(|b| b.to_as_uri());
                    i.zip(o)
                }){
            Some((icon, owner)) => {
                User::from_id(conn, &owner, None, CONFIG.proxy())
                    .await.ok()
                    .and_then(|user| {
                        Media::save_remote(conn, icon, &user).ok().map(|m| m.id)
                    })
            },
            None => None,
        };
        new_blog.icon_id = icon_id;

        let banner_id = match object.image()
                .and_then(|banners| banners.iter().next())
                .and_then(|banner|  banner.to_owned().extend::<Image, ImageType>().ok().flatten() )
                .and_then(|banner|  {
                    let b = banner.url().and_then(|b| b.to_as_uri());
                    let o = banner.attributed_to().and_then(|b| b.to_as_uri());
                    b.zip(o)
                }){
            Some((banner, owner)) => {
                User::from_id(conn, &owner, None, CONFIG.proxy())
                    .await.ok()
                    .and_then(|user| {
                        Media::save_remote(conn, banner, &user).ok().map(|m| m.id)
                    })
            },
            None => None,
        };
        new_blog.banner_id = banner_id;

        new_blog.summary = acct.ext_two.source.content;

        let any_base = AnyBase::from_extended(object)?;
        let id = any_base.id().ok_or(Error::MissingApProperty)?;
        new_blog.ap_url = id.to_string();

        let inst = id
            .authority_components()
            .ok_or(Error::Url)?
            .host()
            .to_string();
        let instance = Instance::find_by_domain(conn, &inst).or_else(|_| {
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
        new_blog.instance_id = instance.id;

        Blog::insert(conn, new_blog)
    }

    fn get_sender() -> &'static dyn sign::Signer {
        Instance::get_local_instance_user().expect("Failed to local instance user")
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
    fn get_key_id(&self) -> String {
        format!("{}#main-key", self.ap_url)
    }

    fn sign(&self, to_sign: &str) -> sign::Result<Vec<u8>> {
        let key = self.get_keypair().map_err(|_| sign::Error())?;
        let mut signer = Signer::new(MessageDigest::sha256(), &key)?;
        signer.update(to_sign.as_bytes())?;
        signer.sign_to_vec().map_err(sign::Error::from)
    }

    fn verify(&self, data: &str, signature: &[u8]) -> sign::Result<bool> {
        let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref())?)?;
        let mut verifier = Verifier::new(MessageDigest::sha256(), &key)?;
        verifier.update(data.as_bytes())?;
        verifier.verify(signature).map_err(sign::Error::from)
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
    use assert_json_diff::assert_json_eq;
    use diesel::Connection;
    use serde_json::to_value;

    pub(crate) fn fill_database(conn: &Conn) -> (Vec<User>, Vec<Blog>) {
        instance_tests::fill_database(conn);
        let users = usersTests::fill_database(conn);
        let mut blog1 = Blog::insert(
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

        blog1.icon_id = Some(
            Media::insert(
                conn,
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
        blog1.banner_id = Some(
            Media::insert(
                conn,
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
        let _: Blog = blog1.save_changes(conn).unwrap();

        (users, vec![blog1, blog2, blog3])
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
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(
                blog.get_instance(conn).unwrap().id,
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
            let (user, _) = fill_database(conn);

            let b1 = Blog::insert(
                conn,
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
                conn,
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
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[1].id,
                    is_owner: false,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[1].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            assert!(blog[0]
                .list_authors(conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[0].id));
            assert!(blog[0]
                .list_authors(conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[1].id));
            assert!(blog[1]
                .list_authors(conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[0].id));
            assert!(!blog[1]
                .list_authors(conn)
                .unwrap()
                .iter()
                .any(|a| a.id == user[1].id));

            assert!(Blog::find_for_author(conn, &user[0])
                .unwrap()
                .iter()
                .any(|b| b.id == blog[0].id));
            assert!(Blog::find_for_author(conn, &user[1])
                .unwrap()
                .iter()
                .any(|b| b.id == blog[0].id));
            assert!(Blog::find_for_author(conn, &user[0])
                .unwrap()
                .iter()
                .any(|b| b.id == blog[1].id));
            assert!(!Blog::find_for_author(conn, &user[1])
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
            fill_database(conn);

            let blog = Blog::insert(
                conn,
                NewBlog::new_local(
                    "SomeName".to_owned(),
                    "Some name".to_owned(),
                    "This is some blog".to_owned(),
                    Instance::get_local().unwrap().id,
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(Blog::find_by_fqn(conn, "SomeName").unwrap().id, blog.id);
            Ok(())
        })
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
            let (_, blogs) = fill_database(conn);

            blogs[0].delete(conn).unwrap();
            assert!(Blog::get(conn, blogs[0].id).is_err());
            Ok(())
        })
    }

    #[test]
    fn delete_via_user() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (user, _) = fill_database(conn);

            let b1 = Blog::insert(
                conn,
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
                conn,
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
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[0].id,
                    author_id: user[1].id,
                    is_owner: false,
                },
            )
            .unwrap();

            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog[1].id,
                    author_id: user[0].id,
                    is_owner: true,
                },
            )
            .unwrap();

            user[0].delete(conn).unwrap();
            assert!(Blog::get(conn, blog[0].id).is_ok());
            assert!(Blog::get(conn, blog[1].id).is_err());
            user[1].delete(conn).unwrap();
            assert!(Blog::get(conn, blog[0].id).is_err());
            Ok(())
        })
    }

    #[test]
    fn self_federation() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, mut blogs) = fill_database(conn);
            blogs[0].icon_id = Some(
                Media::insert(
                    conn,
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
                    conn,
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
            let ap_repr = blogs[0].to_activity(conn).unwrap();
            blogs[0].delete(conn).unwrap();
            let blog = Blog::from_activity(conn, ap_repr).unwrap();

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
            assert_eq!(blog.icon_url(conn), blogs[0].icon_url(conn));
            assert_eq!(blog.banner_url(conn), blogs[0].banner_url(conn));

            Ok(())
        })
    }

    #[test]
    fn to_activity() {
        let conn = &db();
        conn.test_transaction::<_, Error, _>(|| {
            let (_users, blogs) = fill_database(conn);
            let blog = &blogs[0];
            let act = blog.to_activity(conn)?;

            let expected = json!({
                "icon": {
                    "attributedTo": "https://plu.me/@/admin/",
                    "type": "Image",
                    "url": "https://plu.me/aaa.png"
                },
                "id": "https://plu.me/~/BlogName/",
                "image": {
                    "attributedTo": "https://plu.me/@/admin/",
                    "type": "Image",
                    "url": "https://plu.me/bbb.png"
                },
                "inbox": "https://plu.me/~/BlogName/inbox",
                "name": "Blog name",
                "outbox": "https://plu.me/~/BlogName/outbox",
                "preferredUsername": "BlogName",
                "publicKey": {
                    "id": "https://plu.me/~/BlogName/#main-key",
                    "owner": "https://plu.me/~/BlogName/",
                    "publicKeyPem": blog.public_key
                },
                "source": {
                    "content": "This is a small blog",
                    "mediaType": "text/markdown"
                },
                "summary": "",
                "type": "Group"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn outbox_collection() {
        let conn = &db();
        conn.test_transaction::<_, Error, _>(|| {
            let (_users, blogs) = fill_database(conn);
            let blog = &blogs[0];
            let act = blog.outbox_collection(conn)?;

            let expected = json!({
                "items": [],
                "totalItems": 0,
                "first": "https://plu.me/~/BlogName/outbox?page=1",
                "last": "https://plu.me/~/BlogName/outbox?page=0",
                "type": "OrderedCollection"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn outbox_collection_page() {
        let conn = &db();
        conn.test_transaction::<_, Error, _>(|| {
            let (_users, blogs) = fill_database(conn);
            let blog = &blogs[0];
            let act = blog.outbox_collection_page(conn, (33, 36))?;

            let expected = json!({
                "next": "https://plu.me/~/BlogName/outbox?page=3",
                "prev": "https://plu.me/~/BlogName/outbox?page=1",
                "items": [],
                "type": "OrderedCollectionPage"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }
}
