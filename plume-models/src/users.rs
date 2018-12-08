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
use std::cmp::PartialEq;
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
use {ap_url, Connection, BASE_URL, USE_HTTPS};

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
    pub custom_domain: Option<String>,
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
    pub custom_domain: Option<String>,
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

    pub fn one_by_instance(conn: &Connection) -> Vec<User> {
        users::table
            .filter(users::instance_id.eq_any(users::table.select(users::instance_id).distinct()))
            .load::<User>(conn)
            .expect("User::one_by_instance: loading error")
    }

    pub fn delete(&self, conn: &Connection, searcher: &Searcher) {
        use schema::post_authors;

        Blog::find_for_author(conn, self)
            .iter()
            .filter(|b| b.count_authors(conn) <= 1)
            .for_each(|b| b.delete(conn, searcher));
        // delete the posts if they is the only author
        let all_their_posts_ids: Vec<i32> = post_authors::table
            .filter(post_authors::author_id.eq(self.id))
            .select(post_authors::post_id)
            .load(conn)
            .expect("User::delete: post loading error");
        for post_id in all_their_posts_ids {
            let has_other_authors = post_authors::table
                .filter(post_authors::post_id.eq(post_id))
                .filter(post_authors::author_id.ne(self.id))
                .count()
                .load(conn)
                .expect("User::delete: count author error")
                .first()
                .unwrap_or(&0) > &0;
            if !has_other_authors {
                Post::get(conn, post_id)
                    .expect("User::delete: post not found error")
                    .delete(&(conn, searcher));
            }
        }

        diesel::delete(self)
            .execute(conn)
            .expect("User::delete: user deletion error");
    }

    pub fn get_instance(&self, conn: &Connection) -> Instance {
        Instance::get(conn, self.instance_id).expect("User::get_instance: instance not found error")
    }

    pub fn grant_admin_rights(&self, conn: &Connection) {
        diesel::update(self)
            .set(users::is_admin.eq(true))
            .execute(conn)
            .expect("User::grand_admin_rights: update error");
    }

    pub fn revoke_admin_rights(&self, conn: &Connection) {
        diesel::update(self)
            .set(users::is_admin.eq(false))
            .execute(conn)
            .expect("User::grand_admin_rights: update error");
    }

    pub fn update(&self, conn: &Connection, name: String, email: String, summary: String) -> User {
        diesel::update(self)
            .set((
                users::display_name.eq(name),
                users::email.eq(email),
                users::summary.eq(summary),
            ))
            .execute(conn)
            .expect("User::update: update error");
        User::get(conn, self.id).expect("User::update: get error")
    }

    pub fn count_local(conn: &Connection) -> i64 {
        users::table
            .filter(users::instance_id.eq(Instance::local_id(conn)))
            .count()
            .get_result(conn)
            .expect("User::count_local: loading error")
    }

    pub fn find_local(conn: &Connection, username: &str) -> Option<User> {
        User::find_by_name(conn, username, Instance::local_id(conn))
    }

    pub fn find_by_fqn(conn: &Connection, fqn: &str) -> Option<User> {
        if fqn.contains('@') {
            // remote user
            match Instance::find_by_domain(
                conn,
                fqn.split('@')
                    .last()
                    .expect("User::find_by_fqn: host error"),
            ) {
                Some(instance) => match User::find_by_name(
                    conn,
                    fqn.split('@')
                        .nth(0)
                        .expect("User::find_by_fqn: name error")
                    ,
                    instance.id,
                ) {
                    Some(u) => Some(u),
                    None => User::fetch_from_webfinger(conn, fqn),
                },
                None => User::fetch_from_webfinger(conn, fqn),
            }
        } else {
            // local user
            User::find_local(conn, fqn)
        }
    }

    fn fetch_from_webfinger(conn: &Connection, acct: &str) -> Option<User> {
        match resolve(acct.to_owned(), *USE_HTTPS) {
            Ok(wf) => wf
                .links
                .into_iter()
                .find(|l| l.mime_type == Some(String::from("application/activity+json")))
                .and_then(|l| {
                    User::fetch_from_url(
                        conn,
                        &l.href
                            .expect("User::fetch_from_webginfer: href not found error"),
                    )
                }),
            Err(details) => {
                println!("WF Error: {:?}", details);
                None
            }
        }
    }

    fn fetch(url: &str) -> Option<CustomPerson> {
        let req = Client::new()
            .get(url)
            .header(
                ACCEPT,
                HeaderValue::from_str(
                    &ap_accept_header()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", "),
                ).expect("User::fetch: accept header error"),
            )
            .send();
        match req {
            Ok(mut res) => {
                if let Ok(text) = &res.text() {
                    if let Ok(ap_sign) = serde_json::from_str::<ApSignature>(text) {
                        if let Ok(mut json) = serde_json::from_str::<CustomPerson>(text) {
                            json.custom_props = ap_sign; // without this workaround, publicKey is not correctly deserialized
                            Some(json)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(e) => {
                println!("User fetch error: {:?}", e);
                None
            }
        }
    }

    pub fn fetch_from_url(conn: &Connection, url: &str) -> Option<User> {
        User::fetch(url).and_then(|json| {
            (User::from_activity(
                conn,
                &json,
                Url::parse(url)
                    .expect("User::fetch_from_url: url error")
                    .host_str()
                    .expect("User::fetch_from_url: host error"),
            ).ok())
        })
    }

    fn from_activity(conn: &Connection, acct: &CustomPerson, inst: &str) -> Result<User, ()> {
        let instance = match Instance::find_by_domain(conn, inst) {
            Some(instance) => instance,
            None => {
                Instance::insert(
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
                )
            }
        };

        if acct.object.ap_actor_props.preferred_username_string()
            .expect("User::from_activity: preferredUsername error")
            .contains(&['<', '>', '&', '@', '\'', '"'][..]) {
            return Err(());
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
                    .name_string()
                    .expect("User::from_activity: name error"),
                outbox_url: acct
                    .object
                    .ap_actor_props
                    .outbox_string()
                    .expect("User::from_activity: outbox error"),
                inbox_url: acct
                    .object
                    .ap_actor_props
                    .inbox_string()
                    .expect("User::from_activity: inbox error"),
                is_admin: false,
                summary: SafeString::new(
                    &acct
                        .object
                        .object_props
                        .summary_string()
                        .unwrap_or_default(),
                ),
                email: None,
                custom_domain: None,
                hashed_password: None,
                instance_id: instance.id,
                ap_url: acct
                    .object
                    .object_props
                    .id_string()
                    .expect("User::from_activity: id error"),
                public_key: acct
                    .custom_props
                    .public_key_publickey()
                    .expect("User::from_activity: publicKey error")
                    .public_key_pem_string()
                    .expect("User::from_activity: publicKey.publicKeyPem error"),
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
                    .followers_string()
                    .expect("User::from_activity: followers error"),
                avatar_id: None,
            },
        );

        let avatar = Media::save_remote(
            conn,
            acct.object
                .object_props
                .icon_image()
                .expect("User::from_activity: icon error")
                .object_props
                .url_string()
                .expect("User::from_activity: icon.url error"),
            &user,
        );

        if let Ok(avatar) = avatar {
            user.set_avatar(conn, avatar.id);
        }

        Ok(user)
    }

    pub fn refetch(&self, conn: &Connection) {
        User::fetch(&self.ap_url.clone()).map(|json| {
            let avatar = Media::save_remote(
                conn,
                json.object
                    .object_props
                    .icon_image()
                    .expect("User::refetch: icon error")
                    .object_props
                    .url_string()
                    .expect("User::refetch: icon.url error"),
                &self,
            ).ok();

            diesel::update(self)
                .set((
                    users::username.eq(json
                        .object
                        .ap_actor_props
                        .preferred_username_string()
                        .expect("User::refetch: preferredUsername error")),
                    users::display_name.eq(json
                        .object
                        .object_props
                        .name_string()
                        .expect("User::refetch: name error")),
                    users::outbox_url.eq(json
                        .object
                        .ap_actor_props
                        .outbox_string()
                        .expect("User::refetch: outbox error")),
                    users::inbox_url.eq(json
                        .object
                        .ap_actor_props
                        .inbox_string()
                        .expect("User::refetch: inbox error")),
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
                        .followers_string()
                        .expect("User::refetch: followers error")),
                    users::avatar_id.eq(avatar.map(|a| a.id)),
                    users::last_fetched_date.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)
                .expect("User::refetch: update error")
        });
    }

    pub fn hash_pass(pass: &str) -> String {
        bcrypt::hash(pass, 10).expect("User::hash_pass: hashing error")
    }

    pub fn auth(&self, pass: &str) -> bool {
        if let Ok(valid) = bcrypt::verify(
            pass,
            self.hashed_password
                .clone()
                .expect("User::auth: no password error")
                .as_str(),
        ) {
            valid
        } else {
            false
        }
    }

    pub fn update_boxes(&self, conn: &Connection) {
        let instance = self.get_instance(conn);
        if self.outbox_url.is_empty() {
            diesel::update(self)
                .set(users::outbox_url.eq(instance.compute_box(
                    USER_PREFIX,
                    &self.username,
                    "outbox",
                )))
                .execute(conn)
                .expect("User::update_boxes: outbox update error");
        }

        if self.inbox_url.is_empty() {
            diesel::update(self)
                .set(users::inbox_url.eq(instance.compute_box(
                    USER_PREFIX,
                    &self.username,
                    "inbox",
                )))
                .execute(conn)
                .expect("User::update_boxes: inbox update error");
        }

        if self.ap_url.is_empty() {
            diesel::update(self)
                .set(users::ap_url.eq(instance.compute_box(USER_PREFIX, &self.username, "")))
                .execute(conn)
                .expect("User::update_boxes: ap_url update error");
        }

        if self.shared_inbox_url.is_none() {
            diesel::update(self)
                .set(users::shared_inbox_url.eq(ap_url(&format!(
                        "{}/inbox",
                        Instance::get_local(conn)
                            .expect("User::update_boxes: local instance not found error")
                            .public_domain
                    ))))
                .execute(conn)
                .expect("User::update_boxes: shared inbox update error");
        }

        if self.followers_endpoint.is_empty() {
            diesel::update(self)
                .set(users::followers_endpoint.eq(instance.compute_box(
                    USER_PREFIX,
                    &self.username,
                    "followers",
                )))
                .execute(conn)
                .expect("User::update_boxes: follower update error");
        }
    }

    pub fn get_local_page(conn: &Connection, (min, max): (i32, i32)) -> Vec<User> {
        users::table
            .filter(users::instance_id.eq(Instance::local_id(conn)))
            .order(users::username.asc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn)
            .expect("User::get_local_page: loading error")
    }

    pub fn outbox(&self, conn: &Connection) -> ActivityStream<OrderedCollection> {
        let acts = self.get_activities(conn);
        let n_acts = acts.len();
        let mut coll = OrderedCollection::default();
        coll.collection_props.items =
            serde_json::to_value(acts).expect("User::outbox: activity error");
        coll.collection_props
            .set_total_items_u64(n_acts as u64)
            .expect("User::outbox: count error");
        ActivityStream::new(coll)
    }

    pub fn fetch_outbox<T: Activity>(&self) -> Vec<T> {
        let req = Client::new()
            .get(&self.outbox_url[..])
            .header(
                ACCEPT,
                HeaderValue::from_str(
                    &ap_accept_header()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", "),
                ).expect("User::fetch_outbox: accept header error"),
            )
            .send();
        match req {
            Ok(mut res) => {
                let text = &res.text().expect("User::fetch_outbox: body error");
                let json: serde_json::Value =
                    serde_json::from_str(text).expect("User::fetch_outbox: parsing error");
                json["items"]
                    .as_array()
                    .expect("Outbox.items is not an array")
                    .into_iter()
                    .filter_map(|j| serde_json::from_value(j.clone()).ok())
                    .collect::<Vec<T>>()
            }
            Err(e) => {
                println!("User outbox fetch error: {:?}", e);
                vec![]
            }
        }
    }

    pub fn fetch_followers_ids(&self) -> Vec<String> {
        let req = Client::new()
            .get(&self.followers_endpoint[..])
            .header(
                ACCEPT,
                HeaderValue::from_str(
                    &ap_accept_header()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", "),
                ).expect("User::fetch_followers_ids: accept header error"),
            )
            .send();
        match req {
            Ok(mut res) => {
                let text = &res.text().expect("User::fetch_followers_ids: body error");
                let json: serde_json::Value =
                    serde_json::from_str(text).expect("User::fetch_followers_ids: parsing error");
                json["items"]
                    .as_array()
                    .expect("User::fetch_followers_ids: not an array error")
                    .into_iter()
                    .filter_map(|j| serde_json::from_value(j.clone()).ok())
                    .collect::<Vec<String>>()
            }
            Err(e) => {
                println!("User followers fetch error: {:?}", e);
                vec![]
            }
        }
    }

    fn get_activities(&self, conn: &Connection) -> Vec<serde_json::Value> {
        use schema::post_authors;
        use schema::posts;
        let posts_by_self = PostAuthor::belonging_to(self).select(post_authors::post_id);
        let posts = posts::table
            .filter(posts::published.eq(true))
            .filter(posts::id.eq_any(posts_by_self))
            .load::<Post>(conn)
            .expect("User::get_activities: loading error");
        posts
            .into_iter()
            .map(|p| {
                serde_json::to_value(p.create_activity(conn))
                    .expect("User::get_activities: creation error")
            })
            .collect::<Vec<serde_json::Value>>()
    }

    pub fn get_fqn(&self, conn: &Connection) -> String {
        if self.instance_id == Instance::local_id(conn) {
            self.username.clone()
        } else {
            format!(
                "{}@{}",
                self.username,
                self.get_instance(conn).public_domain
            )
        }
    }

    pub fn get_followers(&self, conn: &Connection) -> Vec<User> {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .load::<User>(conn)
            .expect("User::get_followers: loading error")
    }

    pub fn count_followers(&self, conn: &Connection) -> i64 {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .count()
            .get_result(conn)
            .expect("User::count_followers: counting error")
    }

    pub fn get_followers_page(&self, conn: &Connection, (min, max): (i32, i32)) -> Vec<User> {
        use schema::follows;
        let follows = Follow::belonging_to(self).select(follows::follower_id);
        users::table
            .filter(users::id.eq_any(follows))
            .offset(min.into())
            .limit((max - min).into())
            .load::<User>(conn)
            .expect("User::get_followers_page: loading error")
    }

    pub fn get_following(&self, conn: &Connection) -> Vec<User> {
        use schema::follows::dsl::*;
        let f = follows.filter(follower_id.eq(self.id)).select(following_id);
        users::table
            .filter(users::id.eq_any(f))
            .load::<User>(conn)
            .expect("User::get_following: loading error")
    }

    pub fn is_followed_by(&self, conn: &Connection, other_id: i32) -> bool {
        use schema::follows;
        follows::table
            .filter(follows::follower_id.eq(other_id))
            .filter(follows::following_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .expect("User::is_followed_by: loading error") > 0
    }

    pub fn is_following(&self, conn: &Connection, other_id: i32) -> bool {
        use schema::follows;
        follows::table
            .filter(follows::follower_id.eq(self.id))
            .filter(follows::following_id.eq(other_id))
            .count()
            .get_result::<i64>(conn)
            .expect("User::is_following: loading error") > 0
    }

    pub fn has_liked(&self, conn: &Connection, post: &Post) -> bool {
        use schema::likes;
        likes::table
            .filter(likes::post_id.eq(post.id))
            .filter(likes::user_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .expect("User::has_liked: loading error") > 0
    }

    pub fn has_reshared(&self, conn: &Connection, post: &Post) -> bool {
        use schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(post.id))
            .filter(reshares::user_id.eq(self.id))
            .count()
            .get_result::<i64>(conn)
            .expect("User::has_reshared: loading error") > 0
    }

    pub fn is_author_in(&self, conn: &Connection, blog: &Blog) -> bool {
        use schema::blog_authors;
        blog_authors::table
            .filter(blog_authors::author_id.eq(self.id))
            .filter(blog_authors::blog_id.eq(blog.id))
            .count()
            .get_result::<i64>(conn)
            .expect("User::is_author_in: loading error") > 0
    }

    pub fn get_keypair(&self) -> PKey<Private> {
        PKey::from_rsa(
            Rsa::private_key_from_pem(
                self.private_key
                    .clone()
                    .expect("User::get_keypair: private key not found error")
                    .as_ref(),
            ).expect("User::get_keypair: pem parsing error"),
        ).expect("User::get_keypair: private key deserialization error")
    }

    pub fn to_activity(&self, conn: &Connection) -> CustomPerson {
        let mut actor = Person::default();
        actor
            .object_props
            .set_id_string(self.ap_url.clone())
            .expect("User::to_activity: id error");
        actor
            .object_props
            .set_name_string(self.display_name.clone())
            .expect("User::to_activity: name error");
        actor
            .object_props
            .set_summary_string(self.summary.get().clone())
            .expect("User::to_activity: summary error");
        actor
            .object_props
            .set_url_string(self.ap_url.clone())
            .expect("User::to_activity: url error");
        actor
            .ap_actor_props
            .set_inbox_string(self.inbox_url.clone())
            .expect("User::to_activity: inbox error");
        actor
            .ap_actor_props
            .set_outbox_string(self.outbox_url.clone())
            .expect("User::to_activity: outbox error");
        actor
            .ap_actor_props
            .set_preferred_username_string(self.username.clone())
            .expect("User::to_activity: preferredUsername error");
        actor
            .ap_actor_props
            .set_followers_string(self.followers_endpoint.clone())
            .expect("User::to_activity: followers error");

        let mut endpoints = Endpoint::default();
        endpoints
            .set_shared_inbox_string(ap_url(&format!("{}/inbox/", BASE_URL.as_str())))
            .expect("User::to_activity: endpoints.sharedInbox error");
        actor
            .ap_actor_props
            .set_endpoints_endpoint(endpoints)
            .expect("User::to_activity: endpoints error");

        let mut public_key = PublicKey::default();
        public_key
            .set_id_string(format!("{}#main-key", self.ap_url))
            .expect("User::to_activity: publicKey.id error");
        public_key
            .set_owner_string(self.ap_url.clone())
            .expect("User::to_activity: publicKey.owner error");
        public_key
            .set_public_key_pem_string(self.public_key.clone())
            .expect("User::to_activity: publicKey.publicKeyPem error");
        let mut ap_signature = ApSignature::default();
        ap_signature
            .set_public_key_publickey(public_key)
            .expect("User::to_activity: publicKey error");

        let mut avatar = Image::default();
        avatar
            .object_props
            .set_url_string(
                self.avatar_id
                    .and_then(|id| Media::get(conn, id).map(|m| m.url(conn)))
                    .unwrap_or_default(),
            )
            .expect("User::to_activity: icon.url error");
        actor
            .object_props
            .set_icon_object(avatar)
            .expect("User::to_activity: icon error");

        CustomPerson::new(actor, ap_signature)
    }

    pub fn avatar_url(&self, conn: &Connection) -> String {
        self.avatar_id.and_then(|id| Media::get(conn, id).map(|m| m.url(conn))).unwrap_or("/static/default-avatar.png".to_string())
    }

    pub fn webfinger(&self, conn: &Connection) -> Webfinger {
        Webfinger {
            subject: format!(
                "acct:{}@{}",
                self.username,
                self.get_instance(conn).public_domain
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
                    href: Some(self.get_instance(conn).compute_box(
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
        }
    }

    pub fn from_url(conn: &Connection, url: &str) -> Option<User> {
        User::find_by_ap_url(conn, url).or_else(|| {
            // The requested user was not in the DB
            // We try to fetch it if it is remote
            if Url::parse(&url)
                .expect("User::from_url: url error")
                .host_str()
                .expect("User::from_url: host error") != BASE_URL.as_str()
            {
                User::fetch_from_url(conn, url)
            } else {
                None
            }
        })
    }

    pub fn set_avatar(&self, conn: &Connection, id: i32) {
        diesel::update(self)
            .set(users::avatar_id.eq(id))
            .execute(conn)
            .expect("User::set_avatar: update error");
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
            .map(|id| User::get(&*conn, id).expect("User::from_request: user not found error"))
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

    fn sign(&self, to_sign: &str) -> Vec<u8> {
        let key = self.get_keypair();
        let mut signer = sign::Signer::new(MessageDigest::sha256(), &key)
            .expect("User::sign: initialization error");
        signer
            .update(to_sign.as_bytes())
            .expect("User::sign: content insertion error");
        signer
            .sign_to_vec()
            .expect("User::sign: finalization error")
    }

    fn verify(&self, data: &str, signature: &[u8]) -> bool {
        let key = PKey::from_rsa(
            Rsa::public_key_from_pem(self.public_key.as_ref())
                .expect("User::verify: pem parsing error"),
        ).expect("User::verify: deserialization error");
        let mut verifier = sign::Verifier::new(MessageDigest::sha256(), &key)
            .expect("User::verify: initialization error");
        verifier
            .update(data.as_bytes())
            .expect("User::verify: content insertion error");
        verifier
            .verify(&signature)
            .expect("User::verify: finalization error")
    }
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
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
        custom_domain: Option<String>,
        password: String,
    ) -> User {
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
                custom_domain: custom_domain,
                hashed_password: Some(password),
                instance_id: Instance::local_id(conn),
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
            None,
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
            Some("example.com"),
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
            None,
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
