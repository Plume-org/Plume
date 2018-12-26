use activitypub::{
    actor::Person, collection::OrderedCollection, object::Image, Activity, Actor, CustomObject,
    Endpoint, Object,
};
use bcrypt;
use chrono::{NaiveDateTime, Utc};
use diesel::{self, BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    sign,
};
use plume_common::activity_pub::{
    ap_accept_header,
    inbox::{Deletable, WithInbox},
    sign::{gen_keypair, Signer},
    ActivityStream, ApSignature, Id, IntoId, PublicKey,
};
use reqwest::{
    header::{HeaderValue, ACCEPT},
    Client,
};
use rocket::{
    outcome::IntoOutcome,
    request::{self, FromRequest, Request},
};
use serde_json;
use std::{cmp::PartialEq, hash::{Hash, Hasher}};
use url::Url;
use webfinger::*;

use blogs::Blog;
use db_conn::DbConn;
use follows::Follow;
use instance::*;
use medias::Media;
use post_authors::PostAuthor;
use posts::Post;
use safe_string::SafeString;
use schema::users;
use search::Searcher;
use {ap_url, Connection, BASE_URL, USE_HTTPS, Error, Result};

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
    pub last_fetched_date: NaiveDateTime,
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

pub const AUTH_COOKIE: &str = "user_id";
const USER_PREFIX: &str = "@";

impl User {
    insert!(users, NewUser);
    get!(users);
    find_by!(users, find_by_email, email as &str);
    find_by!(users, find_by_name, username as &str, instance_id as i32);
    find_by!(users, find_by_ap_url, ap_url as &str);

    pub fn one_by_instance(conn: &Connection) -> Result<Vec<User>> {
        users::table
            .filter(users::instance_id.eq_any(users::table.select(users::instance_id).distinct()))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn delete(&self, conn: &Connection, searcher: &Searcher) -> Result<usize> {
        use schema::post_authors;

        for blog in Blog::find_for_author(conn, self)?
            .iter()
            .filter(|b| b.count_authors(conn).map(|c| c <= 1).unwrap_or(false)) {
            blog.delete(conn, searcher)?;
        }
        // delete the posts if they is the only author
        let all_their_posts_ids: Vec<i32> = post_authors::table
            .filter(post_authors::author_id.eq(self.id))
            .select(post_authors::post_id)
            .load(conn)?;
        for post_id in all_their_posts_ids {
            let has_other_authors = post_authors::table
                .filter(post_authors::post_id.eq(post_id))
                .filter(post_authors::author_id.ne(self.id))
                .count()
                .load(conn)?
                .first()
                .unwrap_or(&0) > &0;
            if !has_other_authors {
                Post::get(conn, post_id)?
                    .delete(&(conn, searcher))?;
            }
        }

        diesel::delete(self)
            .execute(conn)
            .map_err(Error::from)
    }

    pub fn get_instance(&self, conn: &Connection) -> Result<Instance> {
        Instance::get(conn, self.instance_id)
    }

    pub fn grant_admin_rights(&self, conn: &Connection) -> Result<usize> {
        diesel::update(self)
            .set(users::is_admin.eq(true))
            .execute(conn)
            .map_err(Error::from)
    }

    pub fn revoke_admin_rights(&self, conn: &Connection) -> Result<usize> {
        diesel::update(self)
            .set(users::is_admin.eq(false))
            .execute(conn)
            .map_err(Error::from)
    }

    pub fn update(&self, conn: &Connection, name: String, email: String, summary: String) -> Result<User> {
        diesel::update(self)
            .set((
                users::display_name.eq(name),
                users::email.eq(email),
                users::summary.eq(summary),
            ))
            .execute(conn)?;
        User::get(conn, self.id)
    }

    pub fn count_local(conn: &Connection) -> Result<i64> {
        users::table
            .filter(users::instance_id.eq(Instance::get_local(conn)?.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn find_local(conn: &Connection, username: &str) -> Result<User> {
        User::find_by_name(conn, username, Instance::get_local(conn)?.id)
    }

    pub fn find_by_fqn(conn: &Connection, fqn: &str) -> Result<User> {
        let mut split_fqn = fqn.split('@');
        let username = split_fqn.next().ok_or(Error::InvalidValue)?;
        if let Some(domain) = split_fqn.next() { // remote user
            Instance::find_by_domain(conn, domain)
                .and_then(|instance| User::find_by_name(conn, username, instance.id))
                .or_else(|_| User::fetch_from_webfinger(conn, fqn))
        } else { // local user
            User::find_local(conn, username)
        }
    }

    fn fetch_from_webfinger(conn: &Connection, acct: &str) -> Result<User> {
        let link = resolve(acct.to_owned(), *USE_HTTPS)?
            .links
            .into_iter()
            .find(|l| l.mime_type == Some(String::from("application/activity+json")))
            .ok_or(Error::Webfinger)?;
        User::fetch_from_url(conn, link.href.as_ref()?)
    }

    fn fetch(url: &str) -> Result<CustomPerson> {
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
        // without this workaround, publicKey is not correctly deserialized
        let ap_sign = serde_json::from_str::<ApSignature>(text)?;
        let mut json = serde_json::from_str::<CustomPerson>(text)?;
        json.custom_props = ap_sign;
        Ok(json)
    }

    pub fn fetch_from_url(conn: &Connection, url: &str) -> Result<User> {
        User::fetch(url).and_then(|json| User::from_activity(
            conn,
            &json,
            Url::parse(url)?.host_str()?,
        ))
    }

    fn from_activity(conn: &Connection, acct: &CustomPerson, inst: &str) -> Result<User> {
        let instance = Instance::find_by_domain(conn, inst)
            .or_else(|_| Instance::insert(
                conn,
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
            ))?;

        if acct.object.ap_actor_props.preferred_username_string()?.contains(&['<', '>', '&', '@', '\'', '"'][..]) {
            return Err(Error::InvalidValue);
        }
        let user = User::insert(
            conn,
            NewUser {
                username: acct
                    .object
                    .ap_actor_props
                    .preferred_username_string()
                    .unwrap(),
                display_name: acct
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
                is_admin: false,
                summary: SafeString::new(
                    &acct
                        .object
                        .object_props
                        .summary_string()
                        .unwrap_or_default(),
                ),
                email: None,
                hashed_password: None,
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
                shared_inbox_url: acct
                    .object
                    .ap_actor_props
                    .endpoints_endpoint()
                    .and_then(|e| e.shared_inbox_string())
                    .ok(),
                followers_endpoint: acct
                    .object
                    .ap_actor_props
                    .followers_string()?,
                avatar_id: None,
            },
        )?;

        let avatar = Media::save_remote(
            conn,
            acct.object
                .object_props
                .icon_image()?
                .object_props
                .url_string()?,
            &user,
        );

        if let Ok(avatar) = avatar {
            user.set_avatar(conn, avatar.id)?;
        }

        Ok(user)
    }

    pub fn refetch(&self, conn: &Connection) -> Result<usize> {
        User::fetch(&self.ap_url.clone()).and_then(|json| {
            let avatar = Media::save_remote(
                conn,
                json.object
                    .object_props
                    .icon_image()?
                    .object_props
                    .url_string()?,
                &self,
            ).ok();

            diesel::update(self)
                .set((
                    users::username.eq(json
                        .object
                        .ap_actor_props
                        .preferred_username_string()?),
                    users::display_name.eq(json
                        .object
                        .object_props
                        .name_string()?),
                    users::outbox_url.eq(json
                        .object
                        .ap_actor_props
                        .outbox_string()?),
                    users::inbox_url.eq(json
                        .object
                        .ap_actor_props
                        .inbox_string()?),
                    users::summary.eq(SafeString::new(
                        &json
                            .object
                            .object_props
                            .summary_string()
                            .unwrap_or_default(),
                    )),
                    users::followers_endpoint.eq(json
                        .object
                        .ap_actor_props
                        .followers_string()?),
                    users::avatar_id.eq(avatar.map(|a| a.id)),
                    users::last_fetched_date.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)
                .map_err(Error::from)
        })
    }

    pub fn hash_pass(pass: &str) -> Result<String> {
        bcrypt::hash(pass, 10).map_err(Error::from)
    }

    pub fn auth(&self, pass: &str) -> bool {
        self.hashed_password.clone()
            .map(|hashed| bcrypt::verify(pass, hashed.as_ref()).unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn update_boxes(&self, conn: &Connection) -> Result<()> {
        let instance = self.get_instance(conn)?;
        if self.outbox_url.is_empty() {
            diesel::update(self)
                .set(users::outbox_url.eq(instance.compute_box(
                    USER_PREFIX,
                    &self.username,
                    "outbox",
                )))
                .execute(conn)?;
        }

        if self.inbox_url.is_empty() {
            diesel::update(self)
                .set(users::inbox_url.eq(instance.compute_box(
                    USER_PREFIX,
                    &self.username,
                    "inbox",
                )))
                .execute(conn)?;
        }

        if self.ap_url.is_empty() {
            diesel::update(self)
                .set(users::ap_url.eq(instance.compute_box(USER_PREFIX, &self.username, "")))
                .execute(conn)?;
        }

        if self.shared_inbox_url.is_none() {
            diesel::update(self)
                .set(users::shared_inbox_url.eq(ap_url(&format!(
                        "{}/inbox",
                        Instance::get_local(conn)?
                            .public_domain
                    ))))
                .execute(conn)?;
        }

        if self.followers_endpoint.is_empty() {
            diesel::update(self)
                .set(users::followers_endpoint.eq(instance.compute_box(
                    USER_PREFIX,
                    &self.username,
                    "followers",
                )))
                .execute(conn)?;
        }

        Ok(())
    }

    pub fn get_local_page(conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<User>> {
        users::table
            .filter(users::instance_id.eq(Instance::get_local(conn)?.id))
            .order(users::username.asc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn outbox(&self, conn: &Connection) -> Result<ActivityStream<OrderedCollection>> {
        let acts = self.get_activities(conn)?;
        let n_acts = acts.len();
        let mut coll = OrderedCollection::default();
        coll.collection_props.items = serde_json::to_value(acts)?;
        coll.collection_props
            .set_total_items_u64(n_acts as u64)?;
        Ok(ActivityStream::new(coll))
    }

    pub fn fetch_outbox<T: Activity>(&self) -> Result<Vec<T>> {
        let mut res = Client::new()
            .get(&self.outbox_url[..])
            .header(
                ACCEPT,
                HeaderValue::from_str(
                    &ap_accept_header()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", "),
                )?
            )
            .send()?;
        let text = &res.text()?;
        let json: serde_json::Value =
            serde_json::from_str(text)?;
        Ok(json["items"]
            .as_array()
            .unwrap_or(&vec![])
            .into_iter()
            .filter_map(|j| serde_json::from_value(j.clone()).ok())
            .collect::<Vec<T>>())
    }

    pub fn fetch_followers_ids(&self) -> Result<Vec<String>> {
        let mut res = Client::new()
            .get(&self.followers_endpoint[..])
            .header(
                ACCEPT,
                HeaderValue::from_str(
                    &ap_accept_header()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", "),
                )?
            )
            .send()?;
        let text = &res.text()?;
        let json: serde_json::Value = serde_json::from_str(text)?;
        Ok(json["items"]
            .as_array()
            .unwrap_or(&vec![])
            .into_iter()
            .filter_map(|j| serde_json::from_value(j.clone()).ok())
            .collect::<Vec<String>>())
    }

    fn get_activities(&self, conn: &Connection) -> Result<Vec<serde_json::Value>> {
        use schema::post_authors;
        use schema::posts;
        let posts_by_self = PostAuthor::belonging_to(self).select(post_authors::post_id);
        let posts = posts::table
            .filter(posts::published.eq(true))
            .filter(posts::id.eq_any(posts_by_self))
            .load::<Post>(conn)?;
        Ok(posts
            .into_iter()
            .filter_map(|p| {
                p.create_activity(conn).ok().and_then(|a| serde_json::to_value(a).ok())
            })
            .collect::<Vec<serde_json::Value>>())
    }

    pub fn get_fqn(&self, conn: &Connection) -> String {
        if self.instance_id == Instance::get_local(conn).ok().expect("User::get_fqn: instance error").id {
            self.username.clone()
        } else {
            format!(
                "{}@{}",
                self.username,
                self.get_instance(conn).ok().expect("User::get_fqn: instance error").public_domain
            )
        }
    }

    pub fn get_followers(&self, conn: &Connection) -> Result<Vec<User>> {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn count_followers(&self, conn: &Connection) -> Result<i64> {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_followers_page(&self, conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<User>> {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn get_following(&self, conn: &Connection) -> Result<Vec<User>> {
        use schema::follows::dsl::*;
        let f = follows.filter(follower_id.eq(self.id)).select(following_id);
        users::table
            .filter(users::id.eq_any(f))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn is_followed_by(&self, conn: &Connection, other_id: i32) -> Result<bool> {
        use schema::follows;
        follows::table
            .filter(follows::follower_id.eq(other_id))
            .filter(follows::following_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn is_following(&self, conn: &Connection, other_id: i32) -> Result<bool> {
        use schema::follows;
        follows::table
            .filter(follows::follower_id.eq(self.id))
            .filter(follows::following_id.eq(other_id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn has_liked(&self, conn: &Connection, post: &Post) -> Result<bool> {
        use schema::likes;
        likes::table
            .filter(likes::post_id.eq(post.id))
            .filter(likes::user_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn has_reshared(&self, conn: &Connection, post: &Post) -> Result<bool> {
        use schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(post.id))
            .filter(reshares::user_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
    }

    pub fn is_author_in(&self, conn: &Connection, blog: &Blog) -> Result<bool> {
        use schema::blog_authors;
        blog_authors::table
            .filter(blog_authors::author_id.eq(self.id))
            .filter(blog_authors::blog_id.eq(blog.id))
            .count()
            .get_result::<i64>(conn)
            .map_err(Error::from)
            .map(|r| r > 0)
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

    pub fn to_activity(&self, conn: &Connection) -> Result<CustomPerson> {
        let mut actor = Person::default();
        actor
            .object_props
            .set_id_string(self.ap_url.clone())?;
        actor
            .object_props
            .set_name_string(self.display_name.clone())?;
        actor
            .object_props
            .set_summary_string(self.summary.get().clone())?;
        actor
            .object_props
            .set_url_string(self.ap_url.clone())?;
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

        let mut endpoints = Endpoint::default();
        endpoints
            .set_shared_inbox_string(ap_url(&format!("{}/inbox/", BASE_URL.as_str())))?;
        actor
            .ap_actor_props
            .set_endpoints_endpoint(endpoints)?;

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

        let mut avatar = Image::default();
        avatar
            .object_props
            .set_url_string(
                self.avatar_id
                    .and_then(|id| Media::get(conn, id).and_then(|m| m.url(conn)).ok())
                    .unwrap_or_default(),
            )?;
        actor
            .object_props
            .set_icon_object(avatar)?;

        Ok(CustomPerson::new(actor, ap_signature))
    }

    pub fn avatar_url(&self, conn: &Connection) -> String {
        self.avatar_id.and_then(|id|
            Media::get(conn, id).and_then(|m| m.url(conn)).ok()
        ).unwrap_or("/static/default-avatar.png".to_string())
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
                    mime_type: None,
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
            ],
        })
    }

    pub fn from_url(conn: &Connection, url: &str) -> Result<User> {
        User::find_by_ap_url(conn, url).or_else(|_| {
            // The requested user was not in the DB
            // We try to fetch it if it is remote
            if Url::parse(&url)?.host_str()? != BASE_URL.as_str() {
                User::fetch_from_url(conn, url)
            } else {
                Err(Error::NotFound)
            }
        })
    }

    pub fn set_avatar(&self, conn: &Connection, id: i32) -> Result<usize> {
        diesel::update(self)
            .set(users::avatar_id.eq(id))
            .execute(conn)
            .map_err(Error::from)
    }

    pub fn needs_update(&self) -> bool {
        (Utc::now().naive_utc() - self.last_fetched_date).num_days() > 1
    }

    pub fn name(&self, conn: &Connection) -> String {
        if !self.display_name.is_empty() {
            self.display_name.clone()
        } else {
            self.get_fqn(conn)
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
        Id::new(self.ap_url.clone())
    }
}

impl Eq for User {}
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
    type Error = Error;

    fn get_key_id(&self) -> String {
        format!("{}#main-key", self.ap_url)
    }

    fn sign(&self, to_sign: &str) -> Result<Vec<u8>> {
        let key = self.get_keypair()?;
        let mut signer = sign::Signer::new(MessageDigest::sha256(), &key)?;
        signer
            .update(to_sign.as_bytes())?;
        signer
            .sign_to_vec()
            .map_err(Error::from)
    }

    fn verify(&self, data: &str, signature: &[u8]) -> Result<bool> {
        let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref())?)?;
        let mut verifier = sign::Verifier::new(MessageDigest::sha256(), &key)?;
        verifier
            .update(data.as_bytes())?;
        verifier
            .verify(&signature)
            .map_err(Error::from)
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
        is_admin: bool,
        summary: &str,
        email: String,
        password: String,
    ) -> Result<User> {
        let (pub_key, priv_key) = gen_keypair();
        User::insert(
            conn,
            NewUser {
                username,
                display_name,
                outbox_url: String::from(""),
                inbox_url: String::from(""),
                is_admin,
                summary: SafeString::new(summary),
                email: Some(email),
                hashed_password: Some(password),
                instance_id: Instance::get_local(conn)?.id,
                ap_url: String::from(""),
                public_key: String::from_utf8(pub_key)
                    .expect("NewUser::new_local: public key error"),
                private_key: Some(
                    String::from_utf8(priv_key).expect("NewUser::new_local: private key error"),
                ),
                shared_inbox_url: None,
                followers_endpoint: String::from(""),
                avatar_id: None,
            },
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use diesel::Connection;
    use instance::{tests as instance_tests, Instance};
    use search::tests::get_searcher;
    use tests::db;
    use Connection as Conn;

    pub(crate) fn fill_database(conn: &Conn) -> Vec<User> {
        instance_tests::fill_database(conn);
        let admin = NewUser::new_local(
            conn,
            "admin".to_owned(),
            "The admin".to_owned(),
            true,
            "Hello there, I'm the admin",
            "admin@example.com".to_owned(),
            "invalid_admin_password".to_owned(),
        );
        admin.update_boxes(conn);
        let user = NewUser::new_local(
            conn,
            "user".to_owned(),
            "Some user".to_owned(),
            false,
            "Hello there, I'm no one",
            "user@example.com".to_owned(),
            "invalid_user_password".to_owned(),
        );
        user.update_boxes(conn);
        let other = NewUser::new_local(
            conn,
            "other".to_owned(),
            "Another user".to_owned(),
            false,
            "Hello there, I'm someone else",
            "other@example.com".to_owned(),
            "invalid_other_password".to_owned(),
        );
        other.update_boxes(conn);
        vec![ admin, user, other ]
    }

    #[test]
    fn find_by() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);
            let test_user = NewUser::new_local(
                conn,
                "test".to_owned(),
                "test user".to_owned(),
                false,
                "Hello I'm a test",
                "test@example.com".to_owned(),
                User::hash_pass("test_password"),
            );
            test_user.update_boxes(conn);

            assert_eq!(
                test_user.id,
                User::find_by_name(conn, "test", Instance::local_id(conn))
                    .unwrap()
                    .id
            );
            assert_eq!(
                test_user.id,
                User::find_by_fqn(conn, &test_user.get_fqn(conn)).unwrap().id
            );
            assert_eq!(
                test_user.id,
                User::find_by_email(conn, "test@example.com")
                    .unwrap()
                    .id
            );
            assert_eq!(
                test_user.id,
                User::find_by_ap_url(
                    conn,
                    &format!(
                        "https://{}/@/{}/",
                        Instance::get_local(conn).unwrap().public_domain,
                        "test"
                    )
                ).unwrap()
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

            assert!(User::get(conn, inserted[0].id).is_some());
            inserted[0].delete(conn, &get_searcher());
            assert!(User::get(conn, inserted[0].id).is_none());

            Ok(())
        });
    }

    #[test]
    fn admin() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted = fill_database(conn);
            let local_inst = Instance::get_local(conn).unwrap();
            let mut i = 0;
            while local_inst.has_admin(conn) {
                assert!(i < 100); //prevent from looping indefinitelly
                local_inst.main_admin(conn).revoke_admin_rights(conn);
                i += 1;
            }
            inserted[0].grant_admin_rights(conn);
            assert_eq!(inserted[0].id, local_inst.main_admin(conn).id);

            Ok(())
        });
    }

    #[test]
    fn update() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted = fill_database(conn);
            let updated = inserted[0].update(
                conn,
                "new name".to_owned(),
                "em@il".to_owned(),
                "<p>summary</p><script></script>".to_owned(),
            );
            assert_eq!(updated.display_name, "new name");
            assert_eq!(updated.email.unwrap(), "em@il");
            assert_eq!(updated.summary.get(), "<p>summary</p>");

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
                false,
                "Hello I'm a test",
                "test@example.com".to_owned(),
                User::hash_pass("test_password"),
            );
            test_user.update_boxes(conn);

            assert!(test_user.auth("test_password"));
            assert!(!test_user.auth("other_password"));

            Ok(())
        });
    }

    #[test]
    fn get_local_page() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            fill_database(conn);

            let page = User::get_local_page(conn, (0, 2));
            assert_eq!(page.len(), 2);
            assert!(page[0].username <= page[1].username);

            let mut last_username = User::get_local_page(conn, (0, 1))[0].username.clone();
            for i in 1..User::count_local(conn) as i32 {
                let page = User::get_local_page(conn, (i, i + 1));
                assert_eq!(page.len(), 1);
                assert!(last_username <= page[0].username);
                last_username = page[0].username.clone();
            }
            assert_eq!(
                User::get_local_page(conn, (0, User::count_local(conn) as i32 + 10)).len() as i64,
                User::count_local(conn)
            );

            Ok(())
        });
    }
}
