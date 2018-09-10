use activitypub::{
    activity::{Create, Delete, Update},
    link,
    object::{Article, Tombstone}
};
use chrono::{NaiveDateTime, TimeZone, Utc};
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods, BelongingToDsl, dsl::any};
use heck::KebabCase;
use serde_json;

use plume_common::activity_pub::{
    Hashtag, Source,
    PUBLIC_VISIBILTY, Id, IntoId,
    inbox::{Deletable, FromActivity}
};
use {BASE_URL, ap_url};
use blogs::Blog;
use instance::Instance;
use likes::Like;
use mentions::Mention;
use post_authors::*;
use reshares::Reshare;
use tags::Tag;
use users::User;
use schema::posts;
use safe_string::SafeString;

#[derive(Queryable, Identifiable, Serialize, Clone, AsChangeset)]
pub struct Post {
    pub id: i32,
    pub blog_id: i32,
    pub slug: String,
    pub title: String,
    pub content: SafeString,
    pub published: bool,
    pub license: String,
    pub creation_date: NaiveDateTime,
    pub ap_url: String,
    pub subtitle: String,
    pub source: String,
}

#[derive(Insertable)]
#[table_name = "posts"]
pub struct NewPost {
    pub blog_id: i32,
    pub slug: String,
    pub title: String,
    pub content: SafeString,
    pub published: bool,
    pub license: String,
    pub creation_date: Option<NaiveDateTime>,
    pub ap_url: String,
    pub subtitle: String,
    pub source: String,
}

impl Post {
    insert!(posts, NewPost);
    get!(posts);
    update!(posts);
    find_by!(posts, find_by_slug, slug as String, blog_id as i32);
    find_by!(posts, find_by_ap_url, ap_url as String);

    pub fn list_by_tag(conn: &PgConnection, tag: String, (min, max): (i32, i32)) -> Vec<Post> {
        use schema::tags;

        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table.filter(posts::id.eq(any(ids)))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .get_results::<Post>(conn)
            .expect("Error loading posts by tag")
    }

    pub fn count_for_tag(conn: &PgConnection, tag: String) -> i64 {
        use schema::tags;
        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table.filter(posts::id.eq(any(ids)))
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .expect("Error counting posts by tag")
    }

    pub fn count_local(conn: &PgConnection) -> usize {
        use schema::post_authors;
        use schema::users;
        let local_authors = users::table.filter(users::instance_id.eq(Instance::local_id(conn))).select(users::id);
        let local_posts_id = post_authors::table.filter(post_authors::author_id.eq(any(local_authors))).select(post_authors::post_id);
        posts::table.filter(posts::id.eq(any(local_posts_id)))
            .filter(posts::published.eq(true))
            .load::<Post>(conn)
            .expect("Couldn't load local posts")
            .len()
    }

    pub fn count(conn: &PgConnection) -> i64 {
        posts::table.filter(posts::published.eq(true)).count().get_result(conn).expect("Couldn't count posts")
    }

    pub fn get_recents(conn: &PgConnection, limit: i64) -> Vec<Post> {
        posts::table.order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .limit(limit)
            .load::<Post>(conn)
            .expect("Error loading recent posts")
    }

    pub fn get_recents_for_author(conn: &PgConnection, author: &User, limit: i64) -> Vec<Post> {
        use schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table.filter(posts::id.eq(any(posts)))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .expect("Error loading recent posts for author")
    }

    pub fn get_recents_for_blog(conn: &PgConnection, blog: &Blog, limit: i64) -> Vec<Post> {
        posts::table.filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .expect("Error loading recent posts for blog")
    }

    pub fn get_for_blog(conn: &PgConnection, blog:&Blog) -> Vec<Post> {
        posts::table.filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .load::<Post>(conn)
            .expect("Error loading posts for blog")
    }

    pub fn blog_page(conn: &PgConnection, blog: &Blog, (min, max): (i32, i32)) -> Vec<Post> {
        posts::table.filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Error loading a page of posts for blog")
    }

    /// Give a page of all the recent posts known to this instance (= federated timeline)
    pub fn get_recents_page(conn: &PgConnection, (min, max): (i32, i32)) -> Vec<Post> {
        posts::table.order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Error loading recent posts page")
    }

    /// Give a page of posts from a specific instance
    pub fn get_instance_page(conn: &PgConnection, instance_id: i32, (min, max): (i32, i32)) -> Vec<Post> {
        use schema::blogs;

        let blog_ids = blogs::table.filter(blogs::instance_id.eq(instance_id)).select(blogs::id);

        posts::table.order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .filter(posts::blog_id.eq(any(blog_ids)))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Error loading local posts page")
    }

    /// Give a page of customized user feed, based on a list of followed users
    pub fn user_feed_page(conn: &PgConnection, followed: Vec<i32>, (min, max): (i32, i32)) -> Vec<Post> {
        use schema::post_authors;
        let post_ids = post_authors::table.filter(post_authors::author_id.eq(any(followed)))
            .select(post_authors::post_id);

        posts::table.order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .filter(posts::id.eq(any(post_ids)))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Error loading user feed page")
    }

    pub fn drafts_by_author(conn: &PgConnection, author: &User) -> Vec<Post> {
        use schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table.order(posts::creation_date.desc())
            .filter(posts::published.eq(false))
            .filter(posts::id.eq(any(posts)))
            .load::<Post>(conn)
            .expect("Error listing drafts")
    }

    pub fn get_authors(&self, conn: &PgConnection) -> Vec<User> {
        use schema::users;
        use schema::post_authors;
        let author_list = PostAuthor::belonging_to(self).select(post_authors::author_id);
        users::table.filter(users::id.eq(any(author_list))).load::<User>(conn).unwrap()
    }

    pub fn get_blog(&self, conn: &PgConnection) -> Blog {
        use schema::blogs;
        blogs::table.filter(blogs::id.eq(self.blog_id))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Couldn't load blog associted to post")
            .into_iter().nth(0).unwrap()
    }

    pub fn get_likes(&self, conn: &PgConnection) -> Vec<Like> {
        use schema::likes;
        likes::table.filter(likes::post_id.eq(self.id))
            .load::<Like>(conn)
            .expect("Couldn't load likes associted to post")
    }

    pub fn get_reshares(&self, conn: &PgConnection) -> Vec<Reshare> {
        use schema::reshares;
        reshares::table.filter(reshares::post_id.eq(self.id))
            .load::<Reshare>(conn)
            .expect("Couldn't load reshares associted to post")
    }

    pub fn update_ap_url(&self, conn: &PgConnection) -> Post {
        if self.ap_url.len() == 0 {
            diesel::update(self)
                .set(posts::ap_url.eq(self.compute_id(conn)))
                .get_result::<Post>(conn).expect("Couldn't update AP URL")
        } else {
            self.clone()
        }
    }

    pub fn get_receivers_urls(&self, conn: &PgConnection) -> Vec<String> {
        let followers = self.get_authors(conn).into_iter().map(|a| a.get_followers(conn)).collect::<Vec<Vec<User>>>();
        let to = followers.into_iter().fold(vec![], |mut acc, f| {
            for x in f {
                acc.push(x.ap_url);
            }
            acc
        });
        to
    }

    pub fn into_activity(&self, conn: &PgConnection) -> Article {
        let mut to = self.get_receivers_urls(conn);
        to.push(PUBLIC_VISIBILTY.to_string());

        let mut mentions_json = Mention::list_for_post(conn, self.id).into_iter().map(|m| json!(m.to_activity(conn))).collect::<Vec<serde_json::Value>>();
        let mut tags_json = Tag::for_post(conn, self.id).into_iter().map(|t| json!(t.into_activity(conn))).collect::<Vec<serde_json::Value>>();
        mentions_json.append(&mut tags_json);

        let mut article = Article::default();
        article.object_props.set_name_string(self.title.clone()).expect("Post::into_activity: name error");
        article.object_props.set_id_string(self.ap_url.clone()).expect("Post::into_activity: id error");

        let mut authors = self.get_authors(conn).into_iter().map(|x| Id::new(x.ap_url)).collect::<Vec<Id>>();
        authors.push(self.get_blog(conn).into_id()); // add the blog URL here too
        article.object_props.set_attributed_to_link_vec::<Id>(authors).expect("Post::into_activity: attributedTo error");
        article.object_props.set_content_string(self.content.get().clone()).expect("Post::into_activity: content error");
        article.ap_object_props.set_source_object(Source {
            content: self.source.clone(),
            media_type: String::from("text/markdown"),
        }).expect("Post::into_activity: source error");
        article.object_props.set_published_utctime(Utc.from_utc_datetime(&self.creation_date)).expect("Post::into_activity: published error");
        article.object_props.set_summary_string(self.subtitle.clone()).expect("Post::into_activity: summary error");
        article.object_props.tag = Some(json!(mentions_json));
        article.object_props.set_url_string(self.ap_url.clone()).expect("Post::into_activity: url error");
        article.object_props.set_to_link_vec::<Id>(to.into_iter().map(Id::new).collect()).expect("Post::into_activity: to error");
        article.object_props.set_cc_link_vec::<Id>(vec![]).expect("Post::into_activity: cc error");
        article
    }

    pub fn create_activity(&self, conn: &PgConnection) -> Create {
        let article = self.into_activity(conn);
        let mut act = Create::default();
        act.object_props.set_id_string(format!("{}activity", self.ap_url)).expect("Post::create_activity: id error");
        act.object_props.set_to_link_vec::<Id>(article.object_props.to_link_vec().expect("Post::create_activity: Couldn't copy 'to'"))
            .expect("Post::create_activity: to error");
        act.object_props.set_cc_link_vec::<Id>(article.object_props.cc_link_vec().expect("Post::create_activity: Couldn't copy 'cc'"))
            .expect("Post::create_activity: cc error");
        act.create_props.set_actor_link(Id::new(self.get_authors(conn)[0].clone().ap_url)).expect("Post::create_activity: actor error");
        act.create_props.set_object_object(article).expect("Post::create_activity: object error");
        act
    }

    pub fn update_activity(&self, conn: &PgConnection) -> Update {
        let article = self.into_activity(conn);
        let mut act = Update::default();
        act.object_props.set_id_string(format!("{}/update-{}", self.ap_url, Utc::now().timestamp())).expect("Post::update_activity: id error");
        act.object_props.set_to_link_vec::<Id>(article.object_props.to_link_vec().expect("Post::update_activity: Couldn't copy 'to'"))
            .expect("Post::update_activity: to error");
        act.object_props.set_cc_link_vec::<Id>(article.object_props.cc_link_vec().expect("Post::update_activity: Couldn't copy 'cc'"))
            .expect("Post::update_activity: cc error");
        act.update_props.set_actor_link(Id::new(self.get_authors(conn)[0].clone().ap_url)).expect("Post::update_activity: actor error");
        act.update_props.set_object_object(article).expect("Article::update_activity: object error");
        act
    }

    pub fn handle_update(conn: &PgConnection, updated: Article) {
        let id = updated.object_props.id_string().expect("Post::handle_update: id error");
        let mut post = Post::find_by_ap_url(conn, id).unwrap();

        if let Ok(title) = updated.object_props.name_string() {
            post.slug = title.to_kebab_case();
            post.title = title;
        }

        if let Ok(content) = updated.object_props.content_string() {
            post.content = SafeString::new(&content);
        }

        if let Ok(subtitle) = updated.object_props.summary_string() {
            post.subtitle = subtitle;
        }

        if let Ok(ap_url) = updated.object_props.url_string() {
            post.ap_url = ap_url;
        }

        if let Ok(source) = updated.ap_object_props.source_object::<Source>() {
            post.source = source.content;
        }

        post.update(conn);
    }

    pub fn to_json(&self, conn: &PgConnection) -> serde_json::Value {
        let blog = self.get_blog(conn);
        json!({
            "post": self,
            "author": self.get_authors(conn)[0].to_json(conn),
            "url": format!("/~/{}/{}/", blog.get_fqn(conn), self.slug),
            "date": self.creation_date.timestamp(),
            "blog": blog.to_json(conn),
            "tags": Tag::for_post(&*conn, self.id)
        })
    }

    pub fn compute_id(&self, conn: &PgConnection) -> String {
        ap_url(format!("{}/~/{}/{}/", BASE_URL.as_str(), self.get_blog(conn).get_fqn(conn), self.slug))
    }
}

impl FromActivity<Article, PgConnection> for Post {
    fn from_activity(conn: &PgConnection, article: Article, _actor: Id) -> Post {
        if let Some(post) = Post::find_by_ap_url(conn, article.object_props.id_string().unwrap_or(String::new())) {
            post
        } else {
            let (blog, authors) = article.object_props.attributed_to_link_vec::<Id>()
                .expect("Post::from_activity: attributedTo error")
                .into_iter()
                .fold((None, vec![]), |(blog, mut authors), link| {
                    let url: String = link.into();
                    match User::from_url(conn, url.clone()) {
                        Some(user) => {
                            authors.push(user);
                            (blog, authors)
                        },
                        None => (blog.or_else(|| Blog::from_url(conn, url)), authors)
                    }
                });

            let title = article.object_props.name_string().expect("Post::from_activity: title error");
            let post = Post::insert(conn, NewPost {
                blog_id: blog.expect("Received a new Article without a blog").id,
                slug: title.to_kebab_case(),
                title: title,
                content: SafeString::new(&article.object_props.content_string().expect("Post::from_activity: content error")),
                published: true,
                license: String::from("CC-0"), // TODO
                // FIXME: This is wrong: with this logic, we may use the display URL as the AP ID. We need two different fields
                ap_url: article.object_props.url_string().unwrap_or(article.object_props.id_string().expect("Post::from_activity: url + id error")),
                creation_date: Some(article.object_props.published_utctime().expect("Post::from_activity: published error").naive_utc()),
                subtitle: article.object_props.summary_string().expect("Post::from_activity: summary error"),
                source: article.ap_object_props.source_object::<Source>().expect("Post::from_activity: source error").content
            });

            for author in authors.into_iter() {
                PostAuthor::insert(conn, NewPostAuthor {
                    post_id: post.id,
                    author_id: author.id
                });
            }

            // save mentions and tags
            if let Some(serde_json::Value::Array(tags)) = article.object_props.tag.clone() {
                for tag in tags.into_iter() {
                    serde_json::from_value::<link::Mention>(tag.clone())
                        .map(|m| Mention::from_activity(conn, m, post.id, true, true))
                        .ok();

                    serde_json::from_value::<Hashtag>(tag.clone())
                        .map(|t| Tag::from_activity(conn, t, post.id))
                        .ok();
                }
            }
            post
        }
    }
}

impl Deletable<PgConnection, Delete> for Post {
    fn delete(&self, conn: &PgConnection) -> Delete {
        let mut act = Delete::default();
        act.delete_props.set_actor_link(self.get_authors(conn)[0].clone().into_id()).expect("Post::delete: actor error");

        let mut tombstone = Tombstone::default();
        tombstone.object_props.set_id_string(self.ap_url.clone()).expect("Post::delete: object.id error");
        act.delete_props.set_object_object(tombstone).expect("Post::delete: object error");

        act.object_props.set_id_string(format!("{}#delete", self.ap_url)).expect("Post::delete: id error");
        act.object_props.set_to_link_vec(vec![Id::new(PUBLIC_VISIBILTY)]).expect("Post::delete: to error");

        diesel::delete(self).execute(conn).expect("Post::delete: DB error");
        act
    }

    fn delete_id(id: String, conn: &PgConnection) {
        Post::find_by_ap_url(conn, id).map(|p| p.delete(conn));
    }
}

impl IntoId for Post {
    fn into_id(self) -> Id {
        Id::new(self.ap_url.clone())
    }
}
