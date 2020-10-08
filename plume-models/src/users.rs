use crate::{
    ap_url, blocklisted_emails::BlocklistedEmail, blogs::Blog, config::CONFIG, db_conn::DbConn,
    follows::Follow, instance::*, medias::Media, notifications::Notification,
    post_authors::PostAuthor, posts::Post, safe_string::SafeString, schema::users,
    search::Searcher, timeline::Timeline, Connection, Error, PlumeRocket, Result, ITEMS_PER_PAGE,
};
use activitypub::{
    activity::Delete,
    actor::Person,
    collection::{OrderedCollection, OrderedCollectionPage},
    object::{Image, Tombstone},
    Activity, CustomObject, Endpoint,
};
use bcrypt;
use chrono::{NaiveDateTime, Utc};
use diesel::{self, BelongingToDsl, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use ldap3::{LdapConn, Scope, SearchEntry};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign,
};
use plume_common::{
    activity_pub::{
        ap_accept_header,
        inbox::{AsActor, AsObject, FromId},
        sign::{gen_keypair, Signer},
        ActivityStream, ApSignature, Id, IntoId, PublicKey, PUBLIC_VISIBILITY,
    },
    utils,
};
use reqwest::{
    header::{HeaderValue, ACCEPT},
    ClientBuilder,
};
use rocket::{
    outcome::IntoOutcome,
    request::{self, FromRequest, Request},
};
use serde_json;
use std::{
    cmp::PartialEq,
    hash::{Hash, Hasher},
};
use url::Url;
use webfinger::*;

pub type CustomPerson = CustomObject<ApSignature, Person>;

pub enum Role {
    Admin = 0,
    Moderator = 1,
    Normal = 2,
}

#[derive(Queryable, Identifiable, Clone, Debug, AsChangeset)]
#[changeset_options(treat_none_as_null = "true")]
pub struct User {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub summary: String,
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
    pub last_fetched_date: NaiveDateTime,
    pub fqn: String,
    pub summary_html: SafeString,
    /// 0 = admin
    /// 1 = moderator
    /// anything else = normal user
    pub role: i32,
    pub preferred_theme: Option<String>,
    pub hide_custom_css: bool,
}

#[derive(Default, Insertable)]
#[table_name = "users"]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub summary: String,
    pub email: Option<String>,
    pub hashed_password: Option<String>,
    pub instance_id: i32,
    pub ap_url: String,
    pub private_key: Option<String>,
    pub public_key: String,
    pub shared_inbox_url: Option<String>,
    pub followers_endpoint: String,
    pub avatar_id: Option<i32>,
    pub summary_html: SafeString,
    pub role: i32,
    pub fqn: String,
}

pub const AUTH_COOKIE: &str = "user_id";
const USER_PREFIX: &str = "@";

impl User {
    insert!(users, NewUser);
    get!(users);
    find_by!(users, find_by_email, email as &str);
    find_by!(users, find_by_name, username as &str, instance_id as i32);
    find_by!(users, find_by_ap_url, ap_url as &str);

    pub fn is_moderator(&self) -> bool {
        self.role == Role::Admin as i32 || self.role == Role::Moderator as i32
    }

    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin as i32
    }

    pub fn one_by_instance(conn: &Connection) -> Result<Vec<User>> {
        users::table
            .filter(users::instance_id.eq_any(users::table.select(users::instance_id).distinct()))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn delete(&self, conn: &Connection, searcher: &Searcher) -> Result<()> {
        use crate::schema::post_authors;

        for blog in Blog::find_for_author(conn, self)?
            .iter()
            .filter(|b| b.count_authors(conn).map(|c| c <= 1).unwrap_or(false))
        {
            blog.delete(conn, searcher)?;
        }
        // delete the posts if they is the only author
        let all_their_posts_ids: Vec<i32> = post_authors::table
            .filter(post_authors::author_id.eq(self.id))
            .select(post_authors::post_id)
            .load(conn)?;
        for post_id in all_their_posts_ids {
            // disabling this lint, because otherwise we'd have to turn it on
            // the head, and make it even harder to follow!
            #[allow(clippy::op_ref)]
            let has_other_authors = post_authors::table
                .filter(post_authors::post_id.eq(post_id))
                .filter(post_authors::author_id.ne(self.id))
                .count()
                .load(conn)?
                .first()
                .unwrap_or(&0)
                > &0;
            if !has_other_authors {
                Post::get(conn, post_id)?.delete(conn, searcher)?;
            }
        }

        for notif in Notification::find_followed_by(conn, self)? {
            notif.delete(conn)?
        }

        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn get_instance(&self, conn: &Connection) -> Result<Instance> {
        Instance::get(conn, self.instance_id)
    }

    pub fn set_role(&self, conn: &Connection, new_role: Role) -> Result<()> {
        diesel::update(self)
            .set(users::role.eq(new_role as i32))
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn count_local(conn: &Connection) -> Result<i64> {
        users::table
            .filter(users::instance_id.eq(Instance::get_local()?.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn find_by_fqn(c: &PlumeRocket, fqn: &str) -> Result<User> {
        let from_db = users::table
            .filter(users::fqn.eq(fqn))
            .first(&*c.conn)
            .optional()?;
        if let Some(from_db) = from_db {
            Ok(from_db)
        } else {
            User::fetch_from_webfinger(c, fqn)
        }
    }

    fn fetch_from_webfinger(c: &PlumeRocket, acct: &str) -> Result<User> {
        let link = resolve(acct.to_owned(), true)?
            .links
            .into_iter()
            .find(|l| l.mime_type == Some(String::from("application/activity+json")))
            .ok_or(Error::Webfinger)?;
        User::from_id(c, link.href.as_ref()?, None).map_err(|(_, e)| e)
    }

    pub fn fetch_remote_interact_uri(acct: &str) -> Result<String> {
        resolve(acct.to_owned(), true)?
            .links
            .into_iter()
            .find(|l| l.rel == "http://ostatus.org/schema/1.0/subscribe")
            .and_then(|l| l.template)
            .ok_or(Error::Webfinger)
    }

    fn fetch(url: &str) -> Result<CustomPerson> {
        let mut res = ClientBuilder::new()
            .connect_timeout(Some(std::time::Duration::from_secs(5)))
            .build()?
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
        // without this workaround, publicKey is not correctly deserialized
        let ap_sign = serde_json::from_str::<ApSignature>(text)?;
        let mut json = serde_json::from_str::<CustomPerson>(text)?;
        json.custom_props = ap_sign;
        Ok(json)
    }

    pub fn fetch_from_url(c: &PlumeRocket, url: &str) -> Result<User> {
        User::fetch(url).and_then(|json| User::from_activity(c, json))
    }

    pub fn refetch(&self, conn: &Connection) -> Result<()> {
        User::fetch(&self.ap_url.clone()).and_then(|json| {
            let avatar = Media::save_remote(
                conn,
                json.object
                    .object_props
                    .icon_image()?
                    .object_props
                    .url_string()?,
                &self,
            )
            .ok();

            diesel::update(self)
                .set((
                    users::username.eq(json.object.ap_actor_props.preferred_username_string()?),
                    users::display_name.eq(json.object.object_props.name_string()?),
                    users::outbox_url.eq(json.object.ap_actor_props.outbox_string()?),
                    users::inbox_url.eq(json.object.ap_actor_props.inbox_string()?),
                    users::summary.eq(SafeString::new(
                        &json
                            .object
                            .object_props
                            .summary_string()
                            .unwrap_or_default(),
                    )),
                    users::followers_endpoint.eq(json.object.ap_actor_props.followers_string()?),
                    users::avatar_id.eq(avatar.map(|a| a.id)),
                    users::last_fetched_date.eq(Utc::now().naive_utc()),
                    users::public_key.eq(json
                        .custom_props
                        .public_key_publickey()?
                        .public_key_pem_string()?),
                ))
                .execute(conn)
                .map(|_| ())
                .map_err(Error::from)
        })
    }

    pub fn hash_pass(pass: &str) -> Result<String> {
        bcrypt::hash(pass, 10).map_err(Error::from)
    }

    fn ldap_register(conn: &Connection, name: &str, password: &str) -> Result<User> {
        if CONFIG.ldap.is_none() {
            return Err(Error::NotFound);
        }
        let ldap = CONFIG.ldap.as_ref().unwrap();

        let mut ldap_conn = LdapConn::new(&ldap.addr).map_err(|_| Error::NotFound)?;
        let ldap_name = format!("{}={},{}", ldap.user_name_attr, name, ldap.base_dn);
        let bind = ldap_conn
            .simple_bind(&ldap_name, password)
            .map_err(|_| Error::NotFound)?;

        if bind.success().is_err() {
            return Err(Error::NotFound);
        }

        let search = ldap_conn
            .search(
                &ldap_name,
                Scope::Base,
                "(|(objectClass=person)(objectClass=user))",
                vec![&ldap.mail_attr],
            )
            .map_err(|_| Error::NotFound)?
            .success()
            .map_err(|_| Error::NotFound)?;
        for entry in search.0 {
            let entry = SearchEntry::construct(entry);
            let email = entry.attrs.get("mail").and_then(|vec| vec.first());
            if let Some(email) = email {
                let _ = ldap_conn.unbind();
                return NewUser::new_local(
                    conn,
                    name.to_owned(),
                    name.to_owned(),
                    Role::Normal,
                    "",
                    email.to_owned(),
                    None,
                );
            }
        }
        let _ = ldap_conn.unbind();
        Err(Error::NotFound)
    }

    fn ldap_login(&self, password: &str) -> bool {
        if let Some(ldap) = CONFIG.ldap.as_ref() {
            let mut conn = if let Ok(conn) = LdapConn::new(&ldap.addr) {
                conn
            } else {
                return false;
            };
            let name = format!(
                "{}={},{}",
                ldap.user_name_attr, &self.username, ldap.base_dn
            );
            if let Ok(bind) = conn.simple_bind(&name, password) {
                bind.success().is_ok()
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn login(conn: &Connection, ident: &str, password: &str) -> Result<User> {
        let local_id = Instance::get_local()?.id;
        let user = match User::find_by_email(conn, ident) {
            Ok(user) => Ok(user),
            _ => User::find_by_name(conn, ident, local_id),
        }
        .and_then(|u| {
            if u.instance_id == local_id {
                Ok(u)
            } else {
                Err(Error::NotFound)
            }
        });

        match user {
            Ok(user) if user.hashed_password.is_some() => {
                if bcrypt::verify(password, user.hashed_password.as_ref().unwrap()).unwrap_or(false)
                {
                    Ok(user)
                } else {
                    Err(Error::NotFound)
                }
            }
            Ok(user) => {
                if user.ldap_login(password) {
                    Ok(user)
                } else {
                    Err(Error::NotFound)
                }
            }
            e => {
                if let Ok(user) = User::ldap_register(conn, ident, password) {
                    return Ok(user);
                }
                // if no user was found, and we were unable to auto-register from ldap
                // fake-verify a password, and return an error.
                let other = User::get(&*conn, 1)
                    .expect("No user is registered")
                    .hashed_password;
                other.map(|pass| bcrypt::verify(password, &pass));
                e
            }
        }
    }

    pub fn reset_password(&self, conn: &Connection, pass: &str) -> Result<()> {
        diesel::update(self)
            .set(users::hashed_password.eq(User::hash_pass(pass)?))
            .execute(conn)?;
        Ok(())
    }

    pub fn get_local_page(conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<User>> {
        users::table
            .filter(users::instance_id.eq(Instance::get_local()?.id))
            .order(users::username.asc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn)
            .map_err(Error::from)
    }
    pub fn outbox(&self, conn: &Connection) -> Result<ActivityStream<OrderedCollection>> {
        let mut coll = OrderedCollection::default();
        let first = &format!("{}?page=1", &self.outbox_url);
        let last = &format!(
            "{}?page={}",
            &self.outbox_url,
            self.get_activities_count(&conn) / i64::from(ITEMS_PER_PAGE) + 1
        );
        coll.collection_props.set_first_link(Id::new(first))?;
        coll.collection_props.set_last_link(Id::new(last))?;
        coll.collection_props
            .set_total_items_u64(self.get_activities_count(&conn) as u64)?;
        Ok(ActivityStream::new(coll))
    }
    pub fn outbox_page(
        &self,
        conn: &Connection,
        (min, max): (i32, i32),
    ) -> Result<ActivityStream<OrderedCollectionPage>> {
        let acts = self.get_activities_page(conn, (min, max))?;
        let n_acts = self.get_activities_count(&conn);
        let mut coll = OrderedCollectionPage::default();
        if n_acts - i64::from(min) >= i64::from(ITEMS_PER_PAGE) {
            coll.collection_page_props.set_next_link(Id::new(&format!(
                "{}?page={}",
                &self.outbox_url,
                min / ITEMS_PER_PAGE + 2
            )))?;
        }
        if min > 0 {
            coll.collection_page_props.set_prev_link(Id::new(&format!(
                "{}?page={}",
                &self.outbox_url,
                min / ITEMS_PER_PAGE
            )))?;
        }
        coll.collection_props.items = serde_json::to_value(acts)?;
        coll.collection_page_props
            .set_part_of_link(Id::new(&self.outbox_url))?;
        Ok(ActivityStream::new(coll))
    }
    fn fetch_outbox_page<T: Activity>(&self, url: &str) -> Result<(Vec<T>, Option<String>)> {
        let mut res = ClientBuilder::new()
            .connect_timeout(Some(std::time::Duration::from_secs(5)))
            .build()?
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
        let json: serde_json::Value = serde_json::from_str(text)?;
        let items = json["items"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|j| serde_json::from_value(j.clone()).ok())
            .collect::<Vec<T>>();

        let next = match json.get("next") {
            Some(x) => Some(x.as_str().unwrap().to_owned()),
            None => None,
        };
        Ok((items, next))
    }
    pub fn fetch_outbox<T: Activity>(&self) -> Result<Vec<T>> {
        let mut res = ClientBuilder::new()
            .connect_timeout(Some(std::time::Duration::from_secs(5)))
            .build()?
            .get(&self.outbox_url[..])
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
        let json: serde_json::Value = serde_json::from_str(text)?;
        if let Some(first) = json.get("first") {
            let mut items: Vec<T> = Vec::new();
            let mut next = first.as_str().unwrap().to_owned();
            while let Ok((mut page, nxt)) = self.fetch_outbox_page(&next) {
                if page.is_empty() {
                    break;
                }
                items.extend(page.drain(..));
                if let Some(n) = nxt {
                    if n == next {
                        break;
                    }
                    next = n;
                } else {
                    break;
                }
            }
            Ok(items)
        } else {
            Ok(json["items"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|j| serde_json::from_value(j.clone()).ok())
                .collect::<Vec<T>>())
        }
    }

    pub fn fetch_followers_ids(&self) -> Result<Vec<String>> {
        let mut res = ClientBuilder::new()
            .connect_timeout(Some(std::time::Duration::from_secs(5)))
            .build()?
            .get(&self.followers_endpoint[..])
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
        let json: serde_json::Value = serde_json::from_str(text)?;
        Ok(json["items"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|j| serde_json::from_value(j.clone()).ok())
            .collect::<Vec<String>>())
    }
    fn get_activities_count(&self, conn: &Connection) -> i64 {
        use crate::schema::post_authors;
        use crate::schema::posts;
        let posts_by_self = PostAuthor::belonging_to(self).select(post_authors::post_id);
        posts::table
            .filter(posts::published.eq(true))
            .filter(posts::id.eq_any(posts_by_self))
            .count()
            .first(conn)
            .unwrap()
    }
    fn get_activities_page(
        &self,
        conn: &Connection,
        (min, max): (i32, i32),
    ) -> Result<Vec<serde_json::Value>> {
        use crate::schema::post_authors;
        use crate::schema::posts;
        let posts_by_self = PostAuthor::belonging_to(self).select(post_authors::post_id);
        let posts = posts::table
            .filter(posts::published.eq(true))
            .filter(posts::id.eq_any(posts_by_self))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)?;
        Ok(posts
            .into_iter()
            .filter_map(|p| {
                p.create_activity(conn)
                    .ok()
                    .and_then(|a| serde_json::to_value(a).ok())
            })
            .collect::<Vec<serde_json::Value>>())
    }

    pub fn get_followers(&self, conn: &Connection) -> Result<Vec<User>> {
        use crate::schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn count_followers(&self, conn: &Connection) -> Result<i64> {
        use crate::schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_followers_page(
        &self,
        conn: &Connection,
        (min, max): (i32, i32),
    ) -> Result<Vec<User>> {
        use crate::schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn get_followed(&self, conn: &Connection) -> Result<Vec<User>> {
        use crate::schema::follows::dsl::*;
        let f = follows.filter(follower_id.eq(self.id)).select(following_id);
        users::table
            .filter(users::id.eq_any(f))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn count_followed(&self, conn: &Connection) -> Result<i64> {
        use crate::schema::follows;
        follows::table
            .filter(follows::follower_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_followed_page(
        &self,
        conn: &Connection,
        (min, max): (i32, i32),
    ) -> Result<Vec<User>> {
        use crate::schema::follows;
        let follows = follows::table
            .filter(follows::follower_id.eq(self.id))
            .select(follows::following_id)
            .limit((max - min).into());
        users::table
            .filter(users::id.eq_any(follows))
            .offset(min.into())
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn is_followed_by(&self, conn: &Connection, other_id: i32) -> Result<bool> {
        use crate::schema::follows;
        follows::table
            .filter(follows::follower_id.eq(other_id))
            .filter(follows::following_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn is_following(&self, conn: &Connection, other_id: i32) -> Result<bool> {
        use crate::schema::follows;
        follows::table
            .filter(follows::follower_id.eq(self.id))
            .filter(follows::following_id.eq(other_id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn has_liked(&self, conn: &Connection, post: &Post) -> Result<bool> {
        use crate::schema::likes;
        likes::table
            .filter(likes::post_id.eq(post.id))
            .filter(likes::user_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn has_reshared(&self, conn: &Connection, post: &Post) -> Result<bool> {
        use crate::schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(post.id))
            .filter(reshares::user_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn is_author_in(&self, conn: &Connection, blog: &Blog) -> Result<bool> {
        use crate::schema::blog_authors;
        blog_authors::table
            .filter(blog_authors::author_id.eq(self.id))
            .filter(blog_authors::blog_id.eq(blog.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn get_keypair(&self) -> Result<PKey<Private>> {
        PKey::from_rsa(Rsa::private_key_from_pem(
            self.private_key.clone()?.as_ref(),
        )?)
        .map_err(Error::from)
    }

    pub fn rotate_keypair(&self, conn: &Connection) -> Result<PKey<Private>> {
        if self.private_key.is_none() {
            return Err(Error::InvalidValue);
        }
        if (Utc::now().naive_utc() - self.last_fetched_date).num_minutes() < 10 {
            //rotated recently
            self.get_keypair()
        } else {
            let (public_key, private_key) = gen_keypair();
            let public_key =
                String::from_utf8(public_key).expect("NewUser::new_local: public key error");
            let private_key =
                String::from_utf8(private_key).expect("NewUser::new_local: private key error");
            let res = PKey::from_rsa(Rsa::private_key_from_pem(private_key.as_ref())?)?;
            diesel::update(self)
                .set((
                    users::public_key.eq(public_key),
                    users::private_key.eq(Some(private_key)),
                    users::last_fetched_date.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)
                .map_err(Error::from)
                .map(|_| res)
        }
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<CustomPerson> {
        let mut actor = Person::default();
        actor.object_props.set_id_string(self.ap_url.clone())?;
        actor
            .object_props
            .set_name_string(self.display_name.clone())?;
        actor
            .object_props
            .set_summary_string(self.summary_html.get().clone())?;
        actor.object_props.set_url_string(self.ap_url.clone())?;
        actor
            .ap_actor_props
            .set_inbox_string(self.inbox_url.clone())?;
        actor
            .ap_actor_props
            .set_outbox_string(self.outbox_url.clone())?;
        actor
            .ap_actor_props
            .set_preferred_username_string(self.username.clone())?;
        actor
            .ap_actor_props
            .set_followers_string(self.followers_endpoint.clone())?;

        if let Some(shared_inbox_url) = self.shared_inbox_url.clone() {
            let mut endpoints = Endpoint::default();
            endpoints.set_shared_inbox_string(shared_inbox_url)?;
            actor.ap_actor_props.set_endpoints_endpoint(endpoints)?;
        }

        let mut public_key = PublicKey::default();
        public_key.set_id_string(format!("{}#main-key", self.ap_url))?;
        public_key.set_owner_string(self.ap_url.clone())?;
        public_key.set_public_key_pem_string(self.public_key.clone())?;
        let mut ap_signature = ApSignature::default();
        ap_signature.set_public_key_publickey(public_key)?;

        let mut avatar = Image::default();
        avatar.object_props.set_url_string(
            self.avatar_id
                .and_then(|id| Media::get(conn, id).and_then(|m| m.url()).ok())
                .unwrap_or_default(),
        )?;
        actor.object_props.set_icon_object(avatar)?;

        Ok(CustomPerson::new(actor, ap_signature))
    }

    pub fn delete_activity(&self, conn: &Connection) -> Result<Delete> {
        let mut del = Delete::default();

        let mut tombstone = Tombstone::default();
        tombstone.object_props.set_id_string(self.ap_url.clone())?;

        del.delete_props
            .set_actor_link(Id::new(self.ap_url.clone()))?;
        del.delete_props.set_object_object(tombstone)?;
        del.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        del.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY)])?;
        del.object_props.set_cc_link_vec(
            self.get_followers(conn)?
                .into_iter()
                .map(|f| Id::new(f.ap_url))
                .collect(),
        )?;

        Ok(del)
    }

    pub fn avatar_url(&self, conn: &Connection) -> String {
        self.avatar_id
            .and_then(|id| Media::get(conn, id).and_then(|m| m.url()).ok())
            .unwrap_or_else(|| "/static/images/default-avatar.png".to_string())
    }

    pub fn webfinger(&self, conn: &Connection) -> Result<Webfinger> {
        Ok(Webfinger {
            subject: format!(
                "acct:{}@{}",
                self.username,
                self.get_instance(conn)?.public_domain
            ),
            aliases: vec![self.ap_url.clone()],
            links: vec![
                Link {
                    rel: String::from("http://webfinger.net/rel/profile-page"),
                    mime_type: Some(String::from("text/html")),
                    href: Some(self.ap_url.clone()),
                    template: None,
                },
                Link {
                    rel: String::from("http://schemas.google.com/g/2010#updates-from"),
                    mime_type: Some(String::from("application/atom+xml")),
                    href: Some(self.get_instance(conn)?.compute_box(
                        USER_PREFIX,
                        &self.username,
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
                Link {
                    rel: String::from("http://ostatus.org/schema/1.0/subscribe"),
                    mime_type: None,
                    href: None,
                    template: Some(format!(
                        "https://{}/remote_interact?target={{uri}}",
                        self.get_instance(conn)?.public_domain
                    )),
                },
            ],
        })
    }

    pub fn set_avatar(&self, conn: &Connection, id: i32) -> Result<()> {
        diesel::update(self)
            .set(users::avatar_id.eq(id))
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn needs_update(&self) -> bool {
        (Utc::now().naive_utc() - self.last_fetched_date).num_days() > 1
    }

    pub fn name(&self) -> String {
        if !self.display_name.is_empty() {
            self.display_name.clone()
        } else {
            self.fqn.clone()
        }
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<User, ()> {
        let conn = request.guard::<DbConn>()?;
        request
            .cookies()
            .get_private(AUTH_COOKIE)
            .and_then(|cookie| cookie.value().parse().ok())
            .and_then(|id| User::get(&*conn, id).ok())
            .or_forward(())
    }
}

impl IntoId for User {
    fn into_id(self) -> Id {
        Id::new(self.ap_url)
    }
}

impl Eq for User {}

impl FromId<PlumeRocket> for User {
    type Error = Error;
    type Object = CustomPerson;

    fn from_db(c: &PlumeRocket, id: &str) -> Result<Self> {
        Self::find_by_ap_url(&c.conn, id)
    }

    fn from_activity(c: &PlumeRocket, acct: CustomPerson) -> Result<Self> {
        let url = Url::parse(&acct.object.object_props.id_string()?)?;
        let inst = url.host_str()?;
        let instance = Instance::find_by_domain(&c.conn, inst).or_else(|_| {
            Instance::insert(
                &c.conn,
                NewInstance {
                    name: inst.to_owned(),
                    public_domain: inst.to_owned(),
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

        let username = acct.object.ap_actor_props.preferred_username_string()?;

        if username.contains(&['<', '>', '&', '@', '\'', '"', ' ', '\t'][..]) {
            return Err(Error::InvalidValue);
        }

        let fqn = if instance.local {
            username.clone()
        } else {
            format!("{}@{}", username, instance.public_domain)
        };

        let user = User::insert(
            &c.conn,
            NewUser {
                display_name: acct
                    .object
                    .object_props
                    .name_string()
                    .unwrap_or_else(|_| username.clone()),
                username,
                outbox_url: acct.object.ap_actor_props.outbox_string()?,
                inbox_url: acct.object.ap_actor_props.inbox_string()?,
                role: 2,
                summary: acct
                    .object
                    .object_props
                    .summary_string()
                    .unwrap_or_default(),
                summary_html: SafeString::new(
                    &acct
                        .object
                        .object_props
                        .summary_string()
                        .unwrap_or_default(),
                ),
                email: None,
                hashed_password: None,
                instance_id: instance.id,
                ap_url: acct.object.object_props.id_string()?,
                public_key: acct
                    .custom_props
                    .public_key_publickey()?
                    .public_key_pem_string()?,
                private_key: None,
                shared_inbox_url: acct
                    .object
                    .ap_actor_props
                    .endpoints_endpoint()
                    .and_then(|e| e.shared_inbox_string())
                    .ok(),
                followers_endpoint: acct.object.ap_actor_props.followers_string()?,
                fqn,
                avatar_id: None,
            },
        )?;

        if let Ok(icon) = acct.object.object_props.icon_image() {
            if let Ok(url) = icon.object_props.url_string() {
                let avatar = Media::save_remote(&c.conn, url, &user);

                if let Ok(avatar) = avatar {
                    user.set_avatar(&c.conn, avatar.id)?;
                }
            }
        }

        Ok(user)
    }
}

impl AsActor<&PlumeRocket> for User {
    fn get_inbox_url(&self) -> String {
        self.inbox_url.clone()
    }

    fn get_shared_inbox_url(&self) -> Option<String> {
        self.shared_inbox_url.clone()
    }

    fn is_local(&self) -> bool {
        Instance::get_local()
            .map(|i| self.instance_id == i.id)
            .unwrap_or(false)
    }
}

impl AsObject<User, Delete, &PlumeRocket> for User {
    type Error = Error;
    type Output = ();

    fn activity(self, c: &PlumeRocket, actor: User, _id: &str) -> Result<()> {
        if self.id == actor.id {
            self.delete(&c.conn, &c.searcher).map(|_| ())
        } else {
            Err(Error::Unauthorized)
        }
    }
}

impl Signer for User {
    type Error = Error;

    fn get_key_id(&self) -> String {
        format!("{}#main-key", self.ap_url)
    }

    fn sign(&self, to_sign: &str) -> Result<Vec<u8>> {
        let key = self.get_keypair()?;
        let mut signer = sign::Signer::new(MessageDigest::sha256(), &key)?;
        signer.update(to_sign.as_bytes())?;
        signer.sign_to_vec().map_err(Error::from)
    }

    fn verify(&self, data: &str, signature: &[u8]) -> Result<bool> {
        let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref())?)?;
        let mut verifier = sign::Verifier::new(MessageDigest::sha256(), &key)?;
        verifier.update(data.as_bytes())?;
        verifier.verify(&signature).map_err(Error::from)
    }
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for User {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl NewUser {
    /// Creates a new local user
    pub fn new_local(
        conn: &Connection,
        username: String,
        display_name: String,
        role: Role,
        summary: &str,
        email: String,
        password: Option<String>,
    ) -> Result<User> {
        let (pub_key, priv_key) = gen_keypair();
        let instance = Instance::get_local()?;
        let blocklisted = BlocklistedEmail::matches_blocklist(conn, &email)?;
        if let Some(x) = blocklisted {
            return Err(Error::Blocklisted(x.notify_user, x.notification_text));
        }

        let res = User::insert(
            conn,
            NewUser {
                username: username.clone(),
                display_name,
                role: role as i32,
                summary: summary.to_owned(),
                summary_html: SafeString::new(&utils::md_to_html(&summary, None, false, None).0),
                email: Some(email),
                hashed_password: password,
                instance_id: instance.id,
                public_key: String::from_utf8(pub_key).or(Err(Error::Signature))?,
                private_key: Some(String::from_utf8(priv_key).or(Err(Error::Signature))?),
                outbox_url: instance.compute_box(USER_PREFIX, &username, "outbox"),
                inbox_url: instance.compute_box(USER_PREFIX, &username, "inbox"),
                ap_url: instance.compute_box(USER_PREFIX, &username, ""),
                shared_inbox_url: Some(ap_url(&format!("{}/inbox", &instance.public_domain))),
                followers_endpoint: instance.compute_box(USER_PREFIX, &username, "followers"),
                fqn: username,
                avatar_id: None,
            },
        )?;

        // create default timeline
        Timeline::new_for_user(conn, res.id, "My feed".into(), "followed".into())?;

        Ok(res)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        config::CONFIG,
        instance::{tests as instance_tests, Instance},
        search::tests::get_searcher,
        tests::{db, rockets},
        Connection as Conn,
    };
    use diesel::Connection;

    pub(crate) fn fill_database(conn: &Conn) -> Vec<User> {
        instance_tests::fill_database(conn);
        let admin = NewUser::new_local(
            conn,
            "admin".to_owned(),
            "The admin".to_owned(),
            Role::Admin,
            "Hello there, I'm the admin",
            "admin@example.com".to_owned(),
            Some("invalid_admin_password".to_owned()),
        )
        .unwrap();
        let user = NewUser::new_local(
            conn,
            "user".to_owned(),
            "Some user".to_owned(),
            Role::Normal,
            "Hello there, I'm no one",
            "user@example.com".to_owned(),
            Some("invalid_user_password".to_owned()),
        )
        .unwrap();
        let other = NewUser::new_local(
            conn,
            "other".to_owned(),
            "Another user".to_owned(),
            Role::Normal,
            "Hello there, I'm someone else",
            "other@example.com".to_owned(),
            Some("invalid_other_password".to_owned()),
        )
        .unwrap();
        vec![admin, user, other]
    }

    #[test]
    fn find_by() {
        let r = rockets();
        let conn = &*r.conn;
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);
            let test_user = NewUser::new_local(
                conn,
                "test".to_owned(),
                "test user".to_owned(),
                Role::Normal,
                "Hello I'm a test",
                "test@example.com".to_owned(),
                Some(User::hash_pass("test_password").unwrap()),
            )
            .unwrap();
            assert_eq!(
                test_user.id,
                User::find_by_name(conn, "test", Instance::get_local().unwrap().id)
                    .unwrap()
                    .id
            );
            assert_eq!(
                test_user.id,
                User::find_by_fqn(&r, &test_user.fqn).unwrap().id
            );
            assert_eq!(
                test_user.id,
                User::find_by_email(conn, "test@example.com").unwrap().id
            );
            assert_eq!(
                test_user.id,
                User::find_by_ap_url(
                    conn,
                    &format!(
                        "https://{}/@/{}/",
                        Instance::get_local().unwrap().public_domain,
                        "test"
                    )
                )
                .unwrap()
                .id
            );
            Ok(())
        });
    }

    #[test]
    fn delete() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted = fill_database(conn);

            assert!(User::get(conn, inserted[0].id).is_ok());
            inserted[0]
                .delete(conn, &get_searcher(&CONFIG.search_tokenizers))
                .unwrap();
            assert!(User::get(conn, inserted[0].id).is_err());
            Ok(())
        });
    }

    #[test]
    fn admin() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted = fill_database(conn);
            let local_inst = Instance::get_local().unwrap();
            let mut i = 0;
            while local_inst.has_admin(conn).unwrap() {
                assert!(i < 100); //prevent from looping indefinitelly
                local_inst
                    .main_admin(conn)
                    .unwrap()
                    .set_role(conn, Role::Normal)
                    .unwrap();
                i += 1;
            }
            inserted[0].set_role(conn, Role::Admin).unwrap();
            assert_eq!(inserted[0].id, local_inst.main_admin(conn).unwrap().id);
            Ok(())
        });
    }

    #[test]
    fn auth() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);
            let test_user = NewUser::new_local(
                conn,
                "test".to_owned(),
                "test user".to_owned(),
                Role::Normal,
                "Hello I'm a test",
                "test@example.com".to_owned(),
                Some(User::hash_pass("test_password").unwrap()),
            )
            .unwrap();

            assert_eq!(
                User::login(conn, "test", "test_password").unwrap().id,
                test_user.id
            );
            assert!(User::login(conn, "test", "other_password").is_err());
            Ok(())
        });
    }

    #[test]
    fn get_local_page() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let page = User::get_local_page(conn, (0, 2)).unwrap();
            assert_eq!(page.len(), 2);
            assert!(page[0].username <= page[1].username);

            let mut last_username = User::get_local_page(conn, (0, 1)).unwrap()[0]
                .username
                .clone();
            for i in 1..User::count_local(conn).unwrap() as i32 {
                let page = User::get_local_page(conn, (i, i + 1)).unwrap();
                assert_eq!(page.len(), 1);
                assert!(last_username <= page[0].username);
                last_username = page[0].username.clone();
            }
            assert_eq!(
                User::get_local_page(conn, (0, User::count_local(conn).unwrap() as i32 + 10))
                    .unwrap()
                    .len() as i64,
                User::count_local(conn).unwrap()
            );
            Ok(())
        });
    }

    #[test]
    fn self_federation() {
        let r = rockets();
        let conn = &*r.conn;
        conn.test_transaction::<_, (), _>(|| {
            let users = fill_database(conn);

            let ap_repr = users[0].to_activity(conn).unwrap();
            users[0].delete(conn, &*r.searcher).unwrap();
            let user = User::from_activity(&r, ap_repr).unwrap();

            assert_eq!(user.username, users[0].username);
            assert_eq!(user.display_name, users[0].display_name);
            assert_eq!(user.outbox_url, users[0].outbox_url);
            assert_eq!(user.inbox_url, users[0].inbox_url);
            assert_eq!(user.instance_id, users[0].instance_id);
            assert_eq!(user.ap_url, users[0].ap_url);
            assert_eq!(user.public_key, users[0].public_key);
            assert_eq!(user.shared_inbox_url, users[0].shared_inbox_url);
            assert_eq!(user.followers_endpoint, users[0].followers_endpoint);
            assert_eq!(user.avatar_url(conn), users[0].avatar_url(conn));
            assert_eq!(user.fqn, users[0].fqn);
            assert_eq!(user.summary_html, users[0].summary_html);
            Ok(())
        });
    }
}
