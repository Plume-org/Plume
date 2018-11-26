use activitypub::{
    activity::{Create, Delete, Update},
    link,
    object::{Article, Image, Tombstone},
};
use canapi::{Error, Provider};
use chrono::{NaiveDateTime, TimeZone, Utc};
use diesel::{self, BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use heck::{CamelCase, KebabCase};
use serde_json;

use blogs::Blog;
use instance::Instance;
use likes::Like;
use medias::Media;
use mentions::Mention;
use plume_api::posts::PostEndpoint;
use plume_common::{
    activity_pub::{
        inbox::{Deletable, FromActivity},
        Hashtag, Id, IntoId, Source, PUBLIC_VISIBILTY,
    },
    utils::md_to_html,
};
use post_authors::*;
use reshares::Reshare;
use safe_string::SafeString;
use schema::posts;
use std::collections::HashSet;
use tags::Tag;
use users::User;
use {ap_url, Connection, BASE_URL};

#[derive(Queryable, Identifiable, Serialize, Clone, AsChangeset)]
#[changeset_options(treat_none_as_null = "true")]
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
    pub cover_id: Option<i32>,
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
    pub cover_id: Option<i32>,
}

impl<'a> Provider<(&'a Connection, Option<i32>)> for Post {
    type Data = PostEndpoint;

    fn get(
        (conn, user_id): &(&'a Connection, Option<i32>),
        id: i32,
    ) -> Result<PostEndpoint, Error> {
        if let Some(post) = Post::get(conn, id) {
            if !post.published && !user_id.map(|u| post.is_author(conn, u)).unwrap_or(false) {
                return Err(Error::Authorization(
                    "You are not authorized to access this post yet.".to_string(),
                ));
            }

            Ok(PostEndpoint {
                id: Some(post.id),
                title: Some(post.title.clone()),
                subtitle: Some(post.subtitle.clone()),
                content: Some(post.content.get().clone()),
            })
        } else {
            Err(Error::NotFound("Request post was not found".to_string()))
        }
    }

    fn list(
        (conn, user_id): &(&'a Connection, Option<i32>),
        filter: PostEndpoint,
    ) -> Vec<PostEndpoint> {
        let mut query = posts::table.into_boxed();
        if let Some(title) = filter.title {
            query = query.filter(posts::title.eq(title));
        }
        if let Some(subtitle) = filter.subtitle {
            query = query.filter(posts::subtitle.eq(subtitle));
        }
        if let Some(content) = filter.content {
            query = query.filter(posts::content.eq(content));
        }

        query
            .get_results::<Post>(*conn)
            .map(|ps| {
                ps.into_iter()
                    .filter(|p| {
                        p.published || user_id.map(|u| p.is_author(conn, u)).unwrap_or(false)
                    })
                    .map(|p| PostEndpoint {
                        id: Some(p.id),
                        title: Some(p.title.clone()),
                        subtitle: Some(p.subtitle.clone()),
                        content: Some(p.content.get().clone()),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn create(
        (_conn, _user_id): &(&'a Connection, Option<i32>),
        _query: PostEndpoint,
    ) -> Result<PostEndpoint, Error> {
        unimplemented!()
    }

    fn update(
        (_conn, _user_id): &(&'a Connection, Option<i32>),
        _id: i32,
        _new_data: PostEndpoint,
    ) -> Result<PostEndpoint, Error> {
        unimplemented!()
    }

    fn delete((conn, user_id): &(&'a Connection, Option<i32>), id: i32) {
        let user_id = user_id.expect("Post as Provider::delete: not authenticated");
        if let Some(post) = Post::get(conn, id) {
            if post.is_author(conn, user_id) {
                post.delete(conn);
            }
        }
    }
}

impl Post {
    insert!(posts, NewPost);
    get!(posts);
    update!(posts);
    find_by!(posts, find_by_slug, slug as &str, blog_id as i32);
    find_by!(posts, find_by_ap_url, ap_url as &str);

    pub fn list_by_tag(conn: &Connection, tag: String, (min, max): (i32, i32)) -> Vec<Post> {
        use schema::tags;

        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load(conn)
            .expect("Post::list_by_tag: loading error")
    }

    pub fn count_for_tag(conn: &Connection, tag: String) -> i64 {
        use schema::tags;
        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        *posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .count()
            .load(conn)
            .expect("Post::count_for_tag: counting error")
            .iter()
            .next()
            .expect("Post::count_for_tag: no result error")
    }

    pub fn count_local(conn: &Connection) -> usize {
        use schema::post_authors;
        use schema::users;
        let local_authors = users::table
            .filter(users::instance_id.eq(Instance::local_id(conn)))
            .select(users::id);
        let local_posts_id = post_authors::table
            .filter(post_authors::author_id.eq_any(local_authors))
            .select(post_authors::post_id);
        posts::table
            .filter(posts::id.eq_any(local_posts_id))
            .filter(posts::published.eq(true))
            .load::<Post>(conn)
            .expect("Post::count_local: loading error")
            .len() // TODO count in database?
    }

    pub fn count(conn: &Connection) -> i64 {
        posts::table
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .expect("Post::count: counting error")
    }

    pub fn get_recents(conn: &Connection, limit: i64) -> Vec<Post> {
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .limit(limit)
            .load::<Post>(conn)
            .expect("Post::get_recents: loading error")
    }

    pub fn get_recents_for_author(conn: &Connection, author: &User, limit: i64) -> Vec<Post> {
        use schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table
            .filter(posts::id.eq_any(posts))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .expect("Post::get_recents_for_author: loading error")
    }

    pub fn get_recents_for_blog(conn: &Connection, blog: &Blog, limit: i64) -> Vec<Post> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .expect("Post::get_recents_for_blog: loading error")
    }

    pub fn get_for_blog(conn: &Connection, blog: &Blog) -> Vec<Post> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .load::<Post>(conn)
            .expect("Post::get_for_blog:: loading error")
    }

    pub fn blog_page(conn: &Connection, blog: &Blog, (min, max): (i32, i32)) -> Vec<Post> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Post::blog_page: loading error")
    }

    /// Give a page of all the recent posts known to this instance (= federated timeline)
    pub fn get_recents_page(conn: &Connection, (min, max): (i32, i32)) -> Vec<Post> {
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Post::get_recents_page: loading error")
    }

    /// Give a page of posts from a specific instance
    pub fn get_instance_page(
        conn: &Connection,
        instance_id: i32,
        (min, max): (i32, i32),
    ) -> Vec<Post> {
        use schema::blogs;

        let blog_ids = blogs::table
            .filter(blogs::instance_id.eq(instance_id))
            .select(blogs::id);

        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .filter(posts::blog_id.eq_any(blog_ids))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Post::get_instance_page: loading error")
    }

    /// Give a page of customized user feed, based on a list of followed users
    pub fn user_feed_page(
        conn: &Connection,
        followed: Vec<i32>,
        (min, max): (i32, i32),
    ) -> Vec<Post> {
        use schema::post_authors;
        let post_ids = post_authors::table
            .filter(post_authors::author_id.eq_any(followed))
            .select(post_authors::post_id);

        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .filter(posts::id.eq_any(post_ids))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .expect("Post::user_feed_page: loading error")
    }

    pub fn drafts_by_author(conn: &Connection, author: &User) -> Vec<Post> {
        use schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(false))
            .filter(posts::id.eq_any(posts))
            .load::<Post>(conn)
            .expect("Post::drafts_by_author: loading error")
    }

    pub fn get_authors(&self, conn: &Connection) -> Vec<User> {
        use schema::post_authors;
        use schema::users;
        let author_list = PostAuthor::belonging_to(self).select(post_authors::author_id);
        users::table
            .filter(users::id.eq_any(author_list))
            .load::<User>(conn)
            .expect("Post::get_authors: loading error")
    }

    pub fn is_author(&self, conn: &Connection, author_id: i32) -> bool {
        use schema::post_authors;
        PostAuthor::belonging_to(self)
            .filter(post_authors::author_id.eq(author_id))
            .count()
            .get_result::<i64>(conn)
            .expect("Post::is_author: loading error") > 0
    }

    pub fn get_blog(&self, conn: &Connection) -> Blog {
        use schema::blogs;
        blogs::table
            .filter(blogs::id.eq(self.blog_id))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Post::get_blog: loading error")
            .into_iter()
            .nth(0)
            .expect("Post::get_blog: no result error")
    }

    pub fn get_likes(&self, conn: &Connection) -> Vec<Like> {
        use schema::likes;
        likes::table
            .filter(likes::post_id.eq(self.id))
            .load::<Like>(conn)
            .expect("Post::get_likes: loading error")
    }

    pub fn get_reshares(&self, conn: &Connection) -> Vec<Reshare> {
        use schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(self.id))
            .load::<Reshare>(conn)
            .expect("Post::get_reshares: loading error")
    }

    pub fn update_ap_url(&self, conn: &Connection) -> Post {
        if self.ap_url.is_empty() {
            diesel::update(self)
                .set(posts::ap_url.eq(self.compute_id(conn)))
                .execute(conn)
                .expect("Post::update_ap_url: update error");
            Post::get(conn, self.id).expect("Post::update_ap_url: get error")
        } else {
            self.clone()
        }
    }

    pub fn get_receivers_urls(&self, conn: &Connection) -> Vec<String> {
        let followers = self
            .get_authors(conn)
            .into_iter()
            .map(|a| a.get_followers(conn))
            .collect::<Vec<Vec<User>>>();
        followers.into_iter().fold(vec![], |mut acc, f| {
            for x in f {
                acc.push(x.ap_url);
            }
            acc
        })
    }

    pub fn to_activity(&self, conn: &Connection) -> Article {
        let mut to = self.get_receivers_urls(conn);
        to.push(PUBLIC_VISIBILTY.to_string());

        let mut mentions_json = Mention::list_for_post(conn, self.id)
            .into_iter()
            .map(|m| json!(m.to_activity(conn)))
            .collect::<Vec<serde_json::Value>>();
        let mut tags_json = Tag::for_post(conn, self.id)
            .into_iter()
            .map(|t| json!(t.to_activity(conn)))
            .collect::<Vec<serde_json::Value>>();
        mentions_json.append(&mut tags_json);

        let mut article = Article::default();
        article
            .object_props
            .set_name_string(self.title.clone())
            .expect("Post::to_activity: name error");
        article
            .object_props
            .set_id_string(self.ap_url.clone())
            .expect("Post::to_activity: id error");

        let mut authors = self
            .get_authors(conn)
            .into_iter()
            .map(|x| Id::new(x.ap_url))
            .collect::<Vec<Id>>();
        authors.push(self.get_blog(conn).into_id()); // add the blog URL here too
        article
            .object_props
            .set_attributed_to_link_vec::<Id>(authors)
            .expect("Post::to_activity: attributedTo error");
        article
            .object_props
            .set_content_string(self.content.get().clone())
            .expect("Post::to_activity: content error");
        article
            .ap_object_props
            .set_source_object(Source {
                content: self.source.clone(),
                media_type: String::from("text/markdown"),
            })
            .expect("Post::to_activity: source error");
        article
            .object_props
            .set_published_utctime(Utc.from_utc_datetime(&self.creation_date))
            .expect("Post::to_activity: published error");
        article
            .object_props
            .set_summary_string(self.subtitle.clone())
            .expect("Post::to_activity: summary error");
        article.object_props.tag = Some(json!(mentions_json));

        if let Some(media_id) = self.cover_id {
            let media = Media::get(conn, media_id).expect("Post::to_activity: get cover error");
            let mut cover = Image::default();
            cover
                .object_props
                .set_url_string(media.url(conn))
                .expect("Post::to_activity: icon.url error");
            if media.sensitive {
                cover
                    .object_props
                    .set_summary_string(media.content_warning.unwrap_or_default())
                    .expect("Post::to_activity: icon.summary error");
            }
            cover
                .object_props
                .set_content_string(media.alt_text)
                .expect("Post::to_activity: icon.content error");
            cover
                .object_props
                .set_attributed_to_link_vec(vec![
                    User::get(conn, media.owner_id)
                        .expect("Post::to_activity: media owner not found")
                        .into_id(),
                ])
                .expect("Post::to_activity: icon.attributedTo error");
            article
                .object_props
                .set_icon_object(cover)
                .expect("Post::to_activity: icon error");
        }

        article
            .object_props
            .set_url_string(self.ap_url.clone())
            .expect("Post::to_activity: url error");
        article
            .object_props
            .set_to_link_vec::<Id>(to.into_iter().map(Id::new).collect())
            .expect("Post::to_activity: to error");
        article
            .object_props
            .set_cc_link_vec::<Id>(vec![])
            .expect("Post::to_activity: cc error");
        article
    }

    pub fn create_activity(&self, conn: &Connection) -> Create {
        let article = self.to_activity(conn);
        let mut act = Create::default();
        act.object_props
            .set_id_string(format!("{}activity", self.ap_url))
            .expect("Post::create_activity: id error");
        act.object_props
            .set_to_link_vec::<Id>(
                article
                    .object_props
                    .to_link_vec()
                    .expect("Post::create_activity: Couldn't copy 'to'"),
            )
            .expect("Post::create_activity: to error");
        act.object_props
            .set_cc_link_vec::<Id>(
                article
                    .object_props
                    .cc_link_vec()
                    .expect("Post::create_activity: Couldn't copy 'cc'"),
            )
            .expect("Post::create_activity: cc error");
        act.create_props
            .set_actor_link(Id::new(self.get_authors(conn)[0].clone().ap_url))
            .expect("Post::create_activity: actor error");
        act.create_props
            .set_object_object(article)
            .expect("Post::create_activity: object error");
        act
    }

    pub fn update_activity(&self, conn: &Connection) -> Update {
        let article = self.to_activity(conn);
        let mut act = Update::default();
        act.object_props
            .set_id_string(format!("{}/update-{}", self.ap_url, Utc::now().timestamp()))
            .expect("Post::update_activity: id error");
        act.object_props
            .set_to_link_vec::<Id>(
                article
                    .object_props
                    .to_link_vec()
                    .expect("Post::update_activity: Couldn't copy 'to'"),
            )
            .expect("Post::update_activity: to error");
        act.object_props
            .set_cc_link_vec::<Id>(
                article
                    .object_props
                    .cc_link_vec()
                    .expect("Post::update_activity: Couldn't copy 'cc'"),
            )
            .expect("Post::update_activity: cc error");
        act.update_props
            .set_actor_link(Id::new(self.get_authors(conn)[0].clone().ap_url))
            .expect("Post::update_activity: actor error");
        act.update_props
            .set_object_object(article)
            .expect("Article::update_activity: object error");
        act
    }

    pub fn handle_update(conn: &Connection, updated: &Article) {
        let id = updated
            .object_props
            .id_string()
            .expect("Post::handle_update: id error");
        let mut post = Post::find_by_ap_url(conn, &id).expect("Post::handle_update: finding error");

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

        let mut txt_hashtags = md_to_html(&post.source)
            .2
            .into_iter()
            .map(|s| s.to_camel_case())
            .collect::<HashSet<_>>();
        if let Some(serde_json::Value::Array(mention_tags)) = updated.object_props.tag.clone() {
            let mut mentions = vec![];
            let mut tags = vec![];
            let mut hashtags = vec![];
            for tag in mention_tags {
                serde_json::from_value::<link::Mention>(tag.clone())
                    .map(|m| mentions.push(m))
                    .ok();

                serde_json::from_value::<Hashtag>(tag.clone())
                    .map(|t| {
                        let tag_name = t
                            .name_string()
                            .expect("Post::from_activity: tag name error");
                        if txt_hashtags.remove(&tag_name) {
                            hashtags.push(t);
                        } else {
                            tags.push(t);
                        }
                    })
                    .ok();
            }
            post.update_mentions(conn, mentions);
            post.update_tags(conn, tags);
            post.update_hashtags(conn, hashtags);
        }

        post.update(conn);
    }

    pub fn update_mentions(&self, conn: &Connection, mentions: Vec<link::Mention>) {
        let mentions = mentions
            .into_iter()
            .map(|m| {
                (
                    m.link_props
                        .href_string()
                        .ok()
                        .and_then(|ap_url| User::find_by_ap_url(conn, &ap_url))
                        .map(|u| u.id),
                    m,
                )
            })
            .filter_map(|(id, m)| {
                if let Some(id) = id {
                    Some((m, id))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let old_mentions = Mention::list_for_post(&conn, self.id);
        let old_user_mentioned = old_mentions
            .iter()
            .map(|m| m.mentioned_id)
            .collect::<HashSet<_>>();
        for (m, id) in &mentions {
            if !old_user_mentioned.contains(&id) {
                Mention::from_activity(&*conn, &m, self.id, true, true);
            }
        }

        let new_mentions = mentions
            .into_iter()
            .map(|(_m, id)| id)
            .collect::<HashSet<_>>();
        for m in old_mentions
            .iter()
            .filter(|m| !new_mentions.contains(&m.mentioned_id))
        {
            m.delete(&conn);
        }
    }

    pub fn update_tags(&self, conn: &Connection, tags: Vec<Hashtag>) {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name_string().ok())
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(&*conn, self.id)
            .into_iter()
            .collect::<Vec<_>>();
        let old_tags_name = old_tags
            .iter()
            .filter_map(|tag| {
                if !tag.is_hashtag {
                    Some(tag.tag.clone())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();

        for t in tags {
            if !t
                .name_string()
                .map(|n| old_tags_name.contains(&n))
                .unwrap_or(true)
            {
                Tag::from_activity(conn, &t, self.id, false);
            }
        }

        for ot in old_tags {
            if !tags_name.contains(&ot.tag) {
                ot.delete(conn);
            }
        }
    }

    pub fn update_hashtags(&self, conn: &Connection, tags: Vec<Hashtag>) {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name_string().ok())
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(&*conn, self.id)
            .into_iter()
            .collect::<Vec<_>>();
        let old_tags_name = old_tags
            .iter()
            .filter_map(|tag| {
                if tag.is_hashtag {
                    Some(tag.tag.clone())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();

        for t in tags {
            if !t
                .name_string()
                .map(|n| old_tags_name.contains(&n))
                .unwrap_or(true)
            {
                Tag::from_activity(conn, &t, self.id, true);
            }
        }

        for ot in old_tags {
            if !tags_name.contains(&ot.tag) {
                ot.delete(conn);
            }
        }
    }

    pub fn to_json(&self, conn: &Connection) -> serde_json::Value {
        let blog = self.get_blog(conn);
        json!({
            "post": self,
            "author": self.get_authors(conn)[0].to_json(conn),
            "url": format!("/~/{}/{}/", blog.get_fqn(conn), self.slug),
            "date": self.creation_date.timestamp(),
            "blog": blog.to_json(conn),
            "tags": Tag::for_post(&*conn, self.id),
            "cover": self.cover_id.and_then(|i| Media::get(conn, i).map(|m| m.to_json(conn))),
        })
    }

    pub fn compute_id(&self, conn: &Connection) -> String {
        ap_url(&format!(
            "{}/~/{}/{}/",
            BASE_URL.as_str(),
            self.get_blog(conn).get_fqn(conn),
            self.slug
        ))
    }
}

impl FromActivity<Article, Connection> for Post {
    fn from_activity(conn: &Connection, article: Article, _actor: Id) -> Post {
        if let Some(post) = Post::find_by_ap_url(
            conn,
            &article.object_props.id_string().unwrap_or_default(),
        ) {
            post
        } else {
            let (blog, authors) = article
                .object_props
                .attributed_to_link_vec::<Id>()
                .expect("Post::from_activity: attributedTo error")
                .into_iter()
                .fold((None, vec![]), |(blog, mut authors), link| {
                    let url: String = link.into();
                    match User::from_url(conn, &url) {
                        Some(user) => {
                            authors.push(user);
                            (blog, authors)
                        }
                        None => (blog.or_else(|| Blog::from_url(conn, &url)), authors),
                    }
                });

            let cover = article
                .object_props
                .icon_object::<Image>()
                .ok()
                .and_then(|img| Media::from_activity(conn, &img).map(|m| m.id));

            let title = article
                .object_props
                .name_string()
                .expect("Post::from_activity: title error");
            let post = Post::insert(
                conn,
                NewPost {
                    blog_id: blog.expect("Post::from_activity: blog not found error").id,
                    slug: title.to_kebab_case(),
                    title,
                    content: SafeString::new(
                        &article
                            .object_props
                            .content_string()
                            .expect("Post::from_activity: content error"),
                    ),
                    published: true,
                    license: String::from("CC-BY-SA"), // TODO
                    // FIXME: This is wrong: with this logic, we may use the display URL as the AP ID. We need two different fields
                    ap_url: article.object_props.url_string().unwrap_or_else(|_|
                        article
                            .object_props
                            .id_string()
                            .expect("Post::from_activity: url + id error"),
                    ),
                    creation_date: Some(
                        article
                            .object_props
                            .published_utctime()
                            .expect("Post::from_activity: published error")
                            .naive_utc(),
                    ),
                    subtitle: article
                        .object_props
                        .summary_string()
                        .expect("Post::from_activity: summary error"),
                    source: article
                        .ap_object_props
                        .source_object::<Source>()
                        .expect("Post::from_activity: source error")
                        .content,
                    cover_id: cover,
                },
            );

            for author in authors {
                PostAuthor::insert(
                    conn,
                    NewPostAuthor {
                        post_id: post.id,
                        author_id: author.id,
                    },
                );
            }

            // save mentions and tags
            let mut hashtags = md_to_html(&post.source)
                .2
                .into_iter()
                .map(|s| s.to_camel_case())
                .collect::<HashSet<_>>();
            if let Some(serde_json::Value::Array(tags)) = article.object_props.tag.clone() {
                for tag in tags {
                    serde_json::from_value::<link::Mention>(tag.clone())
                        .map(|m| Mention::from_activity(conn, &m, post.id, true, true))
                        .ok();

                    serde_json::from_value::<Hashtag>(tag.clone())
                        .map(|t| {
                            let tag_name = t
                                .name_string()
                                .expect("Post::from_activity: tag name error");
                            Tag::from_activity(conn, &t, post.id, hashtags.remove(&tag_name));
                        })
                        .ok();
                }
            }
            post
        }
    }
}

impl Deletable<Connection, Delete> for Post {
    fn delete(&self, conn: &Connection) -> Delete {
        let mut act = Delete::default();
        act.delete_props
            .set_actor_link(self.get_authors(conn)[0].clone().into_id())
            .expect("Post::delete: actor error");

        let mut tombstone = Tombstone::default();
        tombstone
            .object_props
            .set_id_string(self.ap_url.clone())
            .expect("Post::delete: object.id error");
        act.delete_props
            .set_object_object(tombstone)
            .expect("Post::delete: object error");

        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))
            .expect("Post::delete: id error");
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILTY)])
            .expect("Post::delete: to error");

        for m in Mention::list_for_post(&conn, self.id) {
            m.delete(conn);
        }
        diesel::delete(self)
            .execute(conn)
            .expect("Post::delete: DB error");
        act
    }

    fn delete_id(id: &str, actor_id: &str, conn: &Connection) {
        let actor = User::find_by_ap_url(conn, actor_id);
        let post = Post::find_by_ap_url(conn, id);
        let can_delete = actor
            .and_then(|act| {
                post.clone()
                    .map(|p| p.get_authors(conn).into_iter().any(|a| act.id == a.id))
            })
            .unwrap_or(false);
        if can_delete {
            post.map(|p| p.delete(conn));
        }
    }
}

impl IntoId for Post {
    fn into_id(self) -> Id {
        Id::new(self.ap_url.clone())
    }
}
