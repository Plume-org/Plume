use crate::{
    ap_url, blogs::Blog, db_conn::DbConn, instance::Instance, medias::Media, mentions::Mention,
    post_authors::*, safe_string::SafeString, schema::posts, tags::*, timeline::*, users::User,
    Connection, Error, PostEvent::*, Result, CONFIG, POST_CHAN,
};
use activitypub::{
    activity::{Create, Delete, Update},
    link,
    object::{Article, Image, Tombstone},
    CustomObject,
};
use chrono::{NaiveDateTime, TimeZone, Utc};
use diesel::{self, BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};
use once_cell::sync::Lazy;
use plume_common::{
    activity_pub::{
        inbox::{AsActor, AsObject, FromId},
        Hashtag, Id, IntoId, Licensed, Source, PUBLIC_VISIBILITY,
    },
    utils::{iri_percent_encode_seg, md_to_html},
};
use riker::actors::{Publish, Tell};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub type LicensedArticle = CustomObject<Licensed, Article>;

static BLOG_FQN_CACHE: Lazy<Mutex<HashMap<i32, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Queryable, Identifiable, Clone, AsChangeset, Debug)]
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

impl Post {
    get!(posts);
    find_by!(posts, find_by_slug, slug as &str, blog_id as i32);
    find_by!(posts, find_by_ap_url, ap_url as &str);

    last!(posts);
    pub fn insert(conn: &Connection, new: NewPost) -> Result<Self> {
        diesel::insert_into(posts::table)
            .values(new)
            .execute(conn)?;
        let mut post = Self::last(conn)?;
        if post.ap_url.is_empty() {
            post.ap_url = Self::ap_url(post.get_blog(conn)?, &post.slug);
            let _: Post = post.save_changes(conn)?;
        }

        if post.published {
            post.publish_published();
        }

        Ok(post)
    }

    pub fn update(&self, conn: &Connection) -> Result<Self> {
        diesel::update(self).set(self).execute(conn)?;
        let post = Self::get(conn, self.id)?;
        // TODO: Call publish_published() when newly published
        if post.published {
            let blog = post.get_blog(conn);
            if blog.is_ok() && blog.unwrap().is_local() {
                self.publish_updated();
            }
        }
        Ok(post)
    }

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        for m in Mention::list_for_post(&conn, self.id)? {
            m.delete(conn)?;
        }
        diesel::delete(self).execute(conn)?;
        self.publish_deleted();
        Ok(())
    }

    pub fn list_by_tag(
        conn: &Connection,
        tag: String,
        (min, max): (i32, i32),
    ) -> Result<Vec<Post>> {
        use crate::schema::tags;

        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load(conn)
            .map_err(Error::from)
    }

    pub fn count_for_tag(conn: &Connection, tag: String) -> Result<i64> {
        use crate::schema::tags;
        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .count()
            .load(conn)?
            .get(0)
            .cloned()
            .ok_or(Error::NotFound)
    }

    pub fn count_local(conn: &Connection) -> Result<i64> {
        use crate::schema::post_authors;
        use crate::schema::users;
        let local_authors = users::table
            .filter(users::instance_id.eq(Instance::get_local()?.id))
            .select(users::id);
        let local_posts_id = post_authors::table
            .filter(post_authors::author_id.eq_any(local_authors))
            .select(post_authors::post_id);
        posts::table
            .filter(posts::id.eq_any(local_posts_id))
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn count(conn: &Connection) -> Result<i64> {
        posts::table
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn list_filtered(
        conn: &Connection,
        title: Option<String>,
        subtitle: Option<String>,
        content: Option<String>,
    ) -> Result<Vec<Post>> {
        let mut query = posts::table.into_boxed();
        if let Some(title) = title {
            query = query.filter(posts::title.eq(title));
        }
        if let Some(subtitle) = subtitle {
            query = query.filter(posts::subtitle.eq(subtitle));
        }
        if let Some(content) = content {
            query = query.filter(posts::content.eq(content));
        }

        query.get_results::<Post>(conn).map_err(Error::from)
    }

    pub fn get_recents_for_author(
        conn: &Connection,
        author: &User,
        limit: i64,
    ) -> Result<Vec<Post>> {
        use crate::schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table
            .filter(posts::id.eq_any(posts))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_recents_for_blog(conn: &Connection, blog: &Blog, limit: i64) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_for_blog(conn: &Connection, blog: &Blog) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn count_for_blog(conn: &Connection, blog: &Blog) -> Result<i64> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn blog_page(conn: &Connection, blog: &Blog, (min, max): (i32, i32)) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn drafts_by_author(conn: &Connection, author: &User) -> Result<Vec<Post>> {
        use crate::schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(false))
            .filter(posts::id.eq_any(posts))
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn ap_url(blog: Blog, slug: &str) -> String {
        ap_url(&format!(
            "{}/~/{}/{}/",
            CONFIG.base_url,
            blog.fqn,
            iri_percent_encode_seg(slug)
        ))
    }

    // It's better to calc slug in insert and update
    pub fn slug(title: &str) -> &str {
        title
    }

    pub fn get_authors(&self, conn: &Connection) -> Result<Vec<User>> {
        use crate::schema::post_authors;
        use crate::schema::users;
        let author_list = PostAuthor::belonging_to(self).select(post_authors::author_id);
        users::table
            .filter(users::id.eq_any(author_list))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn is_author(&self, conn: &Connection, author_id: i32) -> Result<bool> {
        use crate::schema::post_authors;
        Ok(PostAuthor::belonging_to(self)
            .filter(post_authors::author_id.eq(author_id))
            .count()
            .get_result::<i64>(conn)?
            > 0)
    }

    pub fn get_blog(&self, conn: &Connection) -> Result<Blog> {
        use crate::schema::blogs;
        blogs::table
            .filter(blogs::id.eq(self.blog_id))
            .first(conn)
            .map_err(Error::from)
    }

    /// This method exists for use in templates to reduce database access.
    /// This should not be used for other purpose.
    ///
    /// This caches query result. The best way to cache query result is holding it in `Post`s field
    /// but Diesel doesn't allow it currently.
    /// If sometime Diesel allow it, this method should be removed.
    pub fn get_blog_fqn(&self, conn: &Connection) -> String {
        if let Some(blog_fqn) = BLOG_FQN_CACHE.lock().unwrap().get(&self.blog_id) {
            return blog_fqn.to_string();
        }
        let blog_fqn = self.get_blog(conn).unwrap().fqn;
        BLOG_FQN_CACHE
            .lock()
            .unwrap()
            .insert(self.blog_id, blog_fqn.clone());
        blog_fqn
    }

    pub fn count_likes(&self, conn: &Connection) -> Result<i64> {
        use crate::schema::likes;
        likes::table
            .filter(likes::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn count_reshares(&self, conn: &Connection) -> Result<i64> {
        use crate::schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_receivers_urls(&self, conn: &Connection) -> Result<Vec<String>> {
        Ok(self
            .get_authors(conn)?
            .into_iter()
            .filter_map(|a| a.get_followers(conn).ok())
            .fold(vec![], |mut acc, f| {
                for x in f {
                    acc.push(x.ap_url);
                }
                acc
            }))
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<LicensedArticle> {
        let cc = self.get_receivers_urls(conn)?;
        let to = vec![PUBLIC_VISIBILITY.to_string()];

        let mut mentions_json = Mention::list_for_post(conn, self.id)?
            .into_iter()
            .map(|m| json!(m.to_activity(conn).ok()))
            .collect::<Vec<serde_json::Value>>();
        let mut tags_json = Tag::for_post(conn, self.id)?
            .into_iter()
            .map(|t| json!(t.to_activity().ok()))
            .collect::<Vec<serde_json::Value>>();
        mentions_json.append(&mut tags_json);

        let mut article = Article::default();
        article.object_props.set_name_string(self.title.clone())?;
        article.object_props.set_id_string(self.ap_url.clone())?;

        let mut authors = self
            .get_authors(conn)?
            .into_iter()
            .map(|x| Id::new(x.ap_url))
            .collect::<Vec<Id>>();
        authors.push(self.get_blog(conn)?.into_id()); // add the blog URL here too
        article
            .object_props
            .set_attributed_to_link_vec::<Id>(authors)?;
        article
            .object_props
            .set_content_string(self.content.get().clone())?;
        article.ap_object_props.set_source_object(Source {
            content: self.source.clone(),
            media_type: String::from("text/markdown"),
        })?;
        article
            .object_props
            .set_published_utctime(Utc.from_utc_datetime(&self.creation_date))?;
        article
            .object_props
            .set_summary_string(self.subtitle.clone())?;
        article.object_props.tag = Some(json!(mentions_json));

        if let Some(media_id) = self.cover_id {
            let media = Media::get(conn, media_id)?;
            let mut cover = Image::default();
            cover.object_props.set_url_string(media.url()?)?;
            if media.sensitive {
                cover
                    .object_props
                    .set_summary_string(media.content_warning.unwrap_or_default())?;
            }
            cover.object_props.set_content_string(media.alt_text)?;
            cover
                .object_props
                .set_attributed_to_link_vec(vec![User::get(conn, media.owner_id)?.into_id()])?;
            article.object_props.set_icon_object(cover)?;
        }

        article.object_props.set_url_string(self.ap_url.clone())?;
        article
            .object_props
            .set_to_link_vec::<Id>(to.into_iter().map(Id::new).collect())?;
        article
            .object_props
            .set_cc_link_vec::<Id>(cc.into_iter().map(Id::new).collect())?;
        let mut license = Licensed::default();
        license.set_license_string(self.license.clone())?;
        Ok(LicensedArticle::new(article, license))
    }

    pub fn create_activity(&self, conn: &Connection) -> Result<Create> {
        let article = self.to_activity(conn)?;
        let mut act = Create::default();
        act.object_props
            .set_id_string(format!("{}activity", self.ap_url))?;
        act.object_props
            .set_to_link_vec::<Id>(article.object.object_props.to_link_vec()?)?;
        act.object_props
            .set_cc_link_vec::<Id>(article.object.object_props.cc_link_vec()?)?;
        act.create_props
            .set_actor_link(Id::new(self.get_authors(conn)?[0].clone().ap_url))?;
        act.create_props.set_object_object(article)?;
        Ok(act)
    }

    pub fn update_activity(&self, conn: &Connection) -> Result<Update> {
        let article = self.to_activity(conn)?;
        let mut act = Update::default();
        act.object_props.set_id_string(format!(
            "{}/update-{}",
            self.ap_url,
            Utc::now().timestamp()
        ))?;
        act.object_props
            .set_to_link_vec::<Id>(article.object.object_props.to_link_vec()?)?;
        act.object_props
            .set_cc_link_vec::<Id>(article.object.object_props.cc_link_vec()?)?;
        act.update_props
            .set_actor_link(Id::new(self.get_authors(conn)?[0].clone().ap_url))?;
        act.update_props.set_object_object(article)?;
        Ok(act)
    }

    pub fn update_mentions(&self, conn: &Connection, mentions: Vec<link::Mention>) -> Result<()> {
        let mentions = mentions
            .into_iter()
            .map(|m| {
                (
                    m.link_props
                        .href_string()
                        .ok()
                        .and_then(|ap_url| User::find_by_ap_url(conn, &ap_url).ok())
                        .map(|u| u.id),
                    m,
                )
            })
            .filter_map(|(id, m)| id.map(|id| (m, id)))
            .collect::<Vec<_>>();

        let old_mentions = Mention::list_for_post(&conn, self.id)?;
        let old_user_mentioned = old_mentions
            .iter()
            .map(|m| m.mentioned_id)
            .collect::<HashSet<_>>();
        for (m, id) in &mentions {
            if !old_user_mentioned.contains(&id) {
                Mention::from_activity(&*conn, &m, self.id, true, true)?;
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
            m.delete(&conn)?;
        }
        Ok(())
    }

    pub fn update_tags(&self, conn: &Connection, tags: Vec<Hashtag>) -> Result<()> {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name_string().ok())
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(&*conn, self.id)?;
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
                Tag::from_activity(conn, &t, self.id, false)?;
            }
        }

        for ot in old_tags.iter().filter(|t| !t.is_hashtag) {
            if !tags_name.contains(&ot.tag) {
                ot.delete(conn)?;
            }
        }
        Ok(())
    }

    pub fn update_hashtags(&self, conn: &Connection, tags: Vec<Hashtag>) -> Result<()> {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name_string().ok())
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(&*conn, self.id)?;
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
                Tag::from_activity(conn, &t, self.id, true)?;
            }
        }

        for ot in old_tags.into_iter().filter(|t| t.is_hashtag) {
            if !tags_name.contains(&ot.tag) {
                ot.delete(conn)?;
            }
        }
        Ok(())
    }

    pub fn url(&self, conn: &Connection) -> Result<String> {
        let blog = self.get_blog(conn)?;
        Ok(format!("/~/{}/{}", blog.fqn, self.slug))
    }

    pub fn cover_url(&self, conn: &Connection) -> Option<String> {
        self.cover_id
            .and_then(|i| Media::get(conn, i).ok())
            .and_then(|c| c.url().ok())
    }

    pub fn build_delete(&self, conn: &Connection) -> Result<Delete> {
        let mut act = Delete::default();
        act.delete_props
            .set_actor_link(self.get_authors(conn)?[0].clone().into_id())?;

        let mut tombstone = Tombstone::default();
        tombstone.object_props.set_id_string(self.ap_url.clone())?;
        act.delete_props.set_object_object(tombstone)?;

        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY)])?;
        Ok(act)
    }

    fn publish_published(&self) {
        POST_CHAN.tell(
            Publish {
                msg: PostPublished(Arc::new(self.clone())),
                topic: "post.published".into(),
            },
            None,
        )
    }

    fn publish_updated(&self) {
        POST_CHAN.tell(
            Publish {
                msg: PostUpdated(Arc::new(self.clone())),
                topic: "post.updated".into(),
            },
            None,
        )
    }

    fn publish_deleted(&self) {
        POST_CHAN.tell(
            Publish {
                msg: PostDeleted(Arc::new(self.clone())),
                topic: "post.deleted".into(),
            },
            None,
        )
    }
}

impl FromId<DbConn> for Post {
    type Error = Error;
    type Object = LicensedArticle;

    fn from_db(conn: &DbConn, id: &str) -> Result<Self> {
        Self::find_by_ap_url(conn, id)
    }

    fn from_activity(conn: &DbConn, article: LicensedArticle) -> Result<Self> {
        let conn = conn;
        let license = article.custom_props.license_string().unwrap_or_default();
        let article = article.object;

        let (blog, authors) = article
            .object_props
            .attributed_to_link_vec::<Id>()?
            .into_iter()
            .fold((None, vec![]), |(blog, mut authors), link| {
                let url = link;
                match User::from_id(conn, &url, None, CONFIG.proxy()) {
                    Ok(u) => {
                        authors.push(u);
                        (blog, authors)
                    }
                    Err(_) => (
                        blog.or_else(|| Blog::from_id(conn, &url, None, CONFIG.proxy()).ok()),
                        authors,
                    ),
                }
            });

        let cover = article
            .object_props
            .icon_object::<Image>()
            .ok()
            .and_then(|img| Media::from_activity(conn, &img).ok().map(|m| m.id));

        let title = article.object_props.name_string()?;
        let ap_url = article
            .object_props
            .url_string()
            .or_else(|_| article.object_props.id_string())?;
        let post = Post::from_db(conn, &ap_url)
            .and_then(|mut post| {
                let mut updated = false;

                let slug = Self::slug(&title);
                let content = SafeString::new(&article.object_props.content_string()?);
                let subtitle = article.object_props.summary_string()?;
                let source = article.ap_object_props.source_object::<Source>()?.content;
                if post.slug != slug {
                    post.slug = slug.to_string();
                    updated = true;
                }
                if post.title != title {
                    post.title = title.clone();
                    updated = true;
                }
                if post.content != content {
                    post.content = content;
                    updated = true;
                }
                if post.license != license {
                    post.license = license.clone();
                    updated = true;
                }
                if post.subtitle != subtitle {
                    post.subtitle = subtitle;
                    updated = true;
                }
                if post.source != source {
                    post.source = source;
                    updated = true;
                }
                if post.cover_id != cover {
                    post.cover_id = cover;
                    updated = true;
                }

                if updated {
                    post.update(conn)?;
                }

                Ok(post)
            })
            .or_else(|_| {
                Post::insert(
                    conn,
                    NewPost {
                        blog_id: blog?.id,
                        slug: Self::slug(&title).to_string(),
                        title,
                        content: SafeString::new(&article.object_props.content_string()?),
                        published: true,
                        license,
                        // FIXME: This is wrong: with this logic, we may use the display URL as the AP ID. We need two different fields
                        ap_url,
                        creation_date: Some(article.object_props.published_utctime()?.naive_utc()),
                        subtitle: article.object_props.summary_string()?,
                        source: article.ap_object_props.source_object::<Source>()?.content,
                        cover_id: cover,
                    },
                )
                .and_then(|post| {
                    for author in authors {
                        PostAuthor::insert(
                            conn,
                            NewPostAuthor {
                                post_id: post.id,
                                author_id: author.id,
                            },
                        )?;
                    }

                    Ok(post)
                })
            })?;

        // save mentions and tags
        let mut hashtags = md_to_html(&post.source, None, false, None)
            .2
            .into_iter()
            .collect::<HashSet<_>>();
        if let Some(serde_json::Value::Array(tags)) = article.object_props.tag {
            for tag in tags {
                serde_json::from_value::<link::Mention>(tag.clone())
                    .map(|m| Mention::from_activity(conn, &m, post.id, true, true))
                    .ok();

                serde_json::from_value::<Hashtag>(tag.clone())
                    .map_err(Error::from)
                    .and_then(|t| {
                        let tag_name = t.name_string()?;
                        Ok(Tag::from_activity(
                            conn,
                            &t,
                            post.id,
                            hashtags.remove(&tag_name),
                        ))
                    })
                    .ok();
            }
        }

        Timeline::add_to_all_timelines(conn, &post, Kind::Original)?;

        Ok(post)
    }
}

impl AsObject<User, Create, &DbConn> for Post {
    type Error = Error;
    type Output = Post;

    fn activity(self, _conn: &DbConn, _actor: User, _id: &str) -> Result<Post> {
        // TODO: check that _actor is actually one of the author?
        Ok(self)
    }
}

impl AsObject<User, Delete, &DbConn> for Post {
    type Error = Error;
    type Output = ();

    fn activity(self, conn: &DbConn, actor: User, _id: &str) -> Result<()> {
        let can_delete = self
            .get_authors(conn)?
            .into_iter()
            .any(|a| actor.id == a.id);
        if can_delete {
            self.delete(conn).map(|_| ())
        } else {
            Err(Error::Unauthorized)
        }
    }
}

pub struct PostUpdate {
    pub ap_url: String,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    pub cover: Option<i32>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub tags: Option<serde_json::Value>,
}

impl FromId<DbConn> for PostUpdate {
    type Error = Error;
    type Object = LicensedArticle;

    fn from_db(_: &DbConn, _: &str) -> Result<Self> {
        // Always fail because we always want to deserialize the AP object
        Err(Error::NotFound)
    }

    fn from_activity(conn: &DbConn, updated: LicensedArticle) -> Result<Self> {
        Ok(PostUpdate {
            ap_url: updated.object.object_props.id_string()?,
            title: updated.object.object_props.name_string().ok(),
            subtitle: updated.object.object_props.summary_string().ok(),
            content: updated.object.object_props.content_string().ok(),
            cover: updated
                .object
                .object_props
                .icon_object::<Image>()
                .ok()
                .and_then(|img| Media::from_activity(conn, &img).ok().map(|m| m.id)),
            source: updated
                .object
                .ap_object_props
                .source_object::<Source>()
                .ok()
                .map(|x| x.content),
            license: updated.custom_props.license_string().ok(),
            tags: updated.object.object_props.tag,
        })
    }
}

impl AsObject<User, Update, &DbConn> for PostUpdate {
    type Error = Error;
    type Output = ();

    fn activity(self, conn: &DbConn, actor: User, _id: &str) -> Result<()> {
        let mut post =
            Post::from_id(conn, &self.ap_url, None, CONFIG.proxy()).map_err(|(_, e)| e)?;

        if !post.is_author(conn, actor.id)? {
            // TODO: maybe the author was added in the meantime
            return Err(Error::Unauthorized);
        }

        if let Some(title) = self.title {
            post.slug = Post::slug(&title).to_string();
            post.title = title;
        }

        if let Some(content) = self.content {
            post.content = SafeString::new(&content);
        }

        if let Some(subtitle) = self.subtitle {
            post.subtitle = subtitle;
        }

        post.cover_id = self.cover;

        if let Some(source) = self.source {
            post.source = source;
        }

        if let Some(license) = self.license {
            post.license = license;
        }

        let mut txt_hashtags = md_to_html(&post.source, None, false, None)
            .2
            .into_iter()
            .collect::<HashSet<_>>();
        if let Some(serde_json::Value::Array(mention_tags)) = self.tags {
            let mut mentions = vec![];
            let mut tags = vec![];
            let mut hashtags = vec![];
            for tag in mention_tags {
                serde_json::from_value::<link::Mention>(tag.clone())
                    .map(|m| mentions.push(m))
                    .ok();

                serde_json::from_value::<Hashtag>(tag.clone())
                    .map_err(Error::from)
                    .and_then(|t| {
                        let tag_name = t.name_string()?;
                        if txt_hashtags.remove(&tag_name) {
                            hashtags.push(t);
                        } else {
                            tags.push(t);
                        }
                        Ok(())
                    })
                    .ok();
            }
            post.update_mentions(conn, mentions)?;
            post.update_tags(conn, tags)?;
            post.update_hashtags(conn, hashtags)?;
        }

        post.update(conn)?;
        Ok(())
    }
}

impl IntoId for Post {
    fn into_id(self) -> Id {
        Id::new(self.ap_url)
    }
}

#[derive(Clone, Debug)]
pub enum PostEvent {
    PostPublished(Arc<Post>),
    PostUpdated(Arc<Post>),
    PostDeleted(Arc<Post>),
}

impl From<PostEvent> for Arc<Post> {
    fn from(event: PostEvent) -> Self {
        use PostEvent::*;

        match event {
            PostPublished(post) => post,
            PostUpdated(post) => post,
            PostDeleted(post) => post,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::{inbox, tests::fill_database, InboxResult};
    use crate::safe_string::SafeString;
    use crate::tests::db;
    use diesel::Connection;

    // creates a post, get it's Create activity, delete the post,
    // "send" the Create to the inbox, and check it works
    #[test]
    fn self_federation() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, blogs) = fill_database(&conn);
            let post = Post::insert(
                &conn,
                NewPost {
                    blog_id: blogs[0].id,
                    slug: "yo".into(),
                    title: "Yo".into(),
                    content: SafeString::new("Hello"),
                    published: true,
                    license: "WTFPL".to_string(),
                    creation_date: None,
                    ap_url: String::new(), // automatically updated when inserting
                    subtitle: "Testing".into(),
                    source: "Hello".into(),
                    cover_id: None,
                },
            )
            .unwrap();
            PostAuthor::insert(
                &conn,
                NewPostAuthor {
                    post_id: post.id,
                    author_id: users[0].id,
                },
            )
            .unwrap();
            let create = post.create_activity(&conn).unwrap();
            post.delete(&conn).unwrap();

            match inbox(&conn, serde_json::to_value(create).unwrap()).unwrap() {
                InboxResult::Post(p) => {
                    assert!(p.is_author(&conn, users[0].id).unwrap());
                    assert_eq!(p.source, "Hello".to_owned());
                    assert_eq!(p.blog_id, blogs[0].id);
                    assert_eq!(p.content, SafeString::new("Hello"));
                    assert_eq!(p.subtitle, "Testing".to_owned());
                    assert_eq!(p.title, "Yo".to_owned());
                }
                _ => panic!("Unexpected result"),
            };
            Ok(())
        });
    }

    #[test]
    fn licensed_article_serde() {
        let mut article = Article::default();
        article.object_props.set_id_string("Yo".into()).unwrap();
        let mut license = Licensed::default();
        license.set_license_string("WTFPL".into()).unwrap();
        let full_article = LicensedArticle::new(article, license);

        let json = serde_json::to_value(full_article).unwrap();
        let article_from_json: LicensedArticle = serde_json::from_value(json).unwrap();
        assert_eq!(
            "Yo",
            &article_from_json.object.object_props.id_string().unwrap()
        );
        assert_eq!(
            "WTFPL",
            &article_from_json.custom_props.license_string().unwrap()
        );
    }

    #[test]
    fn licensed_article_deserialization() {
        let json = json!({
            "type": "Article",
            "id": "https://plu.me/~/Blog/my-article",
            "attributedTo": ["https://plu.me/@/Admin", "https://plu.me/~/Blog"],
            "content": "Hello.",
            "name": "My Article",
            "summary": "Bye.",
            "source": {
                "content": "Hello.",
                "mediaType": "text/markdown"
            },
            "published": "2014-12-12T12:12:12Z",
            "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
        });
        let article: LicensedArticle = serde_json::from_value(json).unwrap();
        assert_eq!(
            "https://plu.me/~/Blog/my-article",
            &article.object.object_props.id_string().unwrap()
        );
    }
}
