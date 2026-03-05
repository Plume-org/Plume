use crate::{
    ap_url, blogs::Blog, instance::Instance, medias::Media, mentions::Mention, post_authors::*,
    safe_string::SafeString, schema::posts, tags::*, timeline::*, users::User, Connection, Error,
    PostEvent::*, Result, CONFIG, POST_CHAN,
};
use activitystreams::{
    activity::{Create, Delete, Update},
    base::{AnyBase, Base},
    iri_string::types::IriString,
    link::{self, kind::MentionType},
    object::{kind::ImageType, ApObject, Article, AsApObject, Image, ObjectExt, Tombstone},
    prelude::*,
    time::OffsetDateTime,
};
use chrono::{NaiveDateTime, Utc};
use diesel::{self, BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use once_cell::sync::Lazy;
use plume_common::{
    activity_pub::{
        inbox::{AsActor, AsObject, FromId},
        sign::Signer,
        Hashtag, HashtagType, Id, IntoId, Licensed, LicensedArticle, ToAsString, ToAsUri,
        PUBLIC_VISIBILITY,
    },
    utils::{iri_percent_encode_seg, md_to_html},
};
use riker::actors::{Publish, Tell};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

static BLOG_FQN_CACHE: Lazy<Mutex<HashMap<i32, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Queryable, Identifiable, Clone, AsChangeset, Debug)]
#[diesel(treat_none_as_null = true)]
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
#[diesel(table_name = posts)]
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
    pub fn insert(conn: &mut Connection, mut new: NewPost) -> Result<Self> {
        if new.ap_url.is_empty() {
            let blog = Blog::get(conn, new.blog_id)?;
            new.ap_url = Self::ap_url(blog, &new.slug);
        }
        diesel::insert_into(posts::table)
            .values(new)
            .execute(conn)?;
        let post = Self::last(conn)?;

        if post.published {
            post.publish_published();
        }

        Ok(post)
    }

    pub fn update(&self, conn: &mut Connection) -> Result<Self> {
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

    pub fn delete(&self, conn: &mut Connection) -> Result<()> {
        for m in Mention::list_for_post(conn, self.id)? {
            m.delete(conn)?;
        }
        diesel::delete(self).execute(conn)?;
        self.publish_deleted();
        Ok(())
    }

    pub fn list_by_tag(
        conn: &mut Connection,
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

    pub fn count_for_tag(conn: &mut Connection, tag: String) -> Result<i64> {
        use crate::schema::tags;
        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .count()
            .load(conn)?
            .first()
            .cloned()
            .ok_or(Error::NotFound)
    }

    pub fn count_local(conn: &mut Connection) -> Result<i64> {
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

    pub fn count(conn: &mut Connection) -> Result<i64> {
        posts::table
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn list_filtered(
        conn: &mut Connection,
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
        conn: &mut Connection,
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

    pub fn get_recents_for_blog(conn: &mut Connection, blog: &Blog, limit: i64) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_for_blog(conn: &mut Connection, blog: &Blog) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn count_for_blog(conn: &mut Connection, blog: &Blog) -> Result<i64> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn blog_page(conn: &mut Connection, blog: &Blog, (min, max): (i32, i32)) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn drafts_by_author(conn: &mut Connection, author: &User) -> Result<Vec<Post>> {
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
            iri_percent_encode_seg(&blog.fqn),
            iri_percent_encode_seg(slug)
        ))
    }

    // It's better to calc slug in insert and update
    pub fn slug(title: &str) -> &str {
        title
    }

    pub fn get_authors(&self, conn: &mut Connection) -> Result<Vec<User>> {
        use crate::schema::post_authors;
        use crate::schema::users;
        let author_list = PostAuthor::belonging_to(self).select(post_authors::author_id);
        users::table
            .filter(users::id.eq_any(author_list))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn is_author(&self, conn: &mut Connection, author_id: i32) -> Result<bool> {
        use crate::schema::post_authors;
        Ok(PostAuthor::belonging_to(self)
            .filter(post_authors::author_id.eq(author_id))
            .count()
            .get_result::<i64>(conn)?
            > 0)
    }

    pub fn get_blog(&self, conn: &mut Connection) -> Result<Blog> {
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
    pub fn get_blog_fqn(&self, conn: &mut Connection) -> String {
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

    pub fn count_likes(&self, conn: &mut Connection) -> Result<i64> {
        use crate::schema::likes;
        likes::table
            .filter(likes::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn count_reshares(&self, conn: &mut Connection) -> Result<i64> {
        use crate::schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_receivers_urls(&self, conn: &mut Connection) -> Result<Vec<String>> {
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

    pub fn to_activity(&self, conn: &mut Connection) -> Result<LicensedArticle> {
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

        let mut article = ApObject::new(Article::new());
        article.set_name(self.title.clone());
        article.set_id(self.ap_url.parse::<IriString>()?);

        let mut authors = self
            .get_authors(conn)?
            .into_iter()
            .filter_map(|x| x.ap_url.parse::<IriString>().ok())
            .collect::<Vec<IriString>>();
        authors.push(self.get_blog(conn)?.ap_url.parse::<IriString>()?); // add the blog URL here too
        article.set_many_attributed_tos(authors);
        article.set_content(self.content.get().clone());
        let source = AnyBase::from_arbitrary_json(serde_json::json!({
            "content": self.source,
            "mediaType": "text/markdown",
        }))?;
        article.set_source(source);
        article.set_published(
            self.creation_date.and_utc().timestamp_nanos_opt()
                .and_then(|cd| OffsetDateTime::from_unix_timestamp_nanos(cd.into()).ok())
                .expect("OffsetDateTime")
        );
        article.set_summary(&*self.subtitle);
        article.set_many_tags(
            mentions_json
                .iter()
                .filter_map(|mention_json| AnyBase::from_arbitrary_json(mention_json).ok()),
        );

        if let Some(media_id) = self.cover_id {
            let media = Media::get(conn, media_id)?;
            let mut cover = Image::new();
            cover.set_url(media.url()?);
            if media.sensitive {
                cover.set_summary(media.content_warning.unwrap_or_default());
            }
            cover.set_content(media.alt_text);
            cover.set_many_attributed_tos(vec![User::get(conn, media.owner_id)?
                .ap_url
                .parse::<IriString>()?]);
            article.set_icon(cover.into_any_base()?);
        }

        article.set_url(self.ap_url.parse::<IriString>()?);
        article.set_many_tos(
            to.into_iter()
                .filter_map(|to| to.parse::<IriString>().ok())
                .collect::<Vec<IriString>>(),
        );
        article.set_many_ccs(
            cc.into_iter()
                .filter_map(|cc| cc.parse::<IriString>().ok())
                .collect::<Vec<IriString>>(),
        );
        let license = Licensed {
            license: Some(self.license.clone()),
        };
        Ok(LicensedArticle::new(article, license))
    }

    pub fn create_activity(&self, conn: &mut Connection) -> Result<Create> {
        let article = self.to_activity(conn)?;
        let to = article.to().ok_or(Error::MissingApProperty)?.clone();
        let cc = article.cc().ok_or(Error::MissingApProperty)?.clone();
        let mut act = Create::new(
            self.get_authors(conn)?[0].ap_url.parse::<IriString>()?,
            Base::retract(article)?.into_generic()?,
        );
        act.set_id(format!("{}/activity", self.ap_url).parse::<IriString>()?);
        act.set_many_tos(to);
        act.set_many_ccs(cc);
        Ok(act)
    }

    pub fn update_activity(&self, conn: &mut Connection) -> Result<Update> {
        let article = self.to_activity(conn)?;
        let to = article.to().ok_or(Error::MissingApProperty)?.clone();
        let cc = article.cc().ok_or(Error::MissingApProperty)?.clone();
        let mut act = Update::new(
            self.get_authors(conn)?[0].ap_url.parse::<IriString>()?,
            Base::retract(article)?.into_generic()?,
        );
        act.set_id(
            format!("{}/update-{}", self.ap_url, Utc::now().timestamp()).parse::<IriString>()?,
        );
        act.set_many_tos(to);
        act.set_many_ccs(cc);
        Ok(act)
    }

    pub fn update_mentions(&self, conn: &mut Connection, mentions: Vec<link::Mention>) -> Result<()> {
        let mentions = mentions
            .into_iter()
            .map(|m| {
                (
                    m.href()
                        .and_then(|ap_url| User::find_by_ap_url(conn, ap_url.as_ref()).ok())
                        .map(|u| u.id),
                    m,
                )
            })
            .filter_map(|(id, m)| id.map(|id| (m, id)))
            .collect::<Vec<_>>();

        let old_mentions = Mention::list_for_post(conn, self.id)?;
        let old_user_mentioned = old_mentions
            .iter()
            .map(|m| m.mentioned_id)
            .collect::<HashSet<_>>();
        for (m, id) in &mentions {
            if !old_user_mentioned.contains(id) {
                Mention::from_activity(conn, m, self.id, true, true)?;
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
            m.delete(conn)?;
        }
        Ok(())
    }

    pub fn update_tags(&self, conn: &mut Connection, tags: Vec<Hashtag>) -> Result<()> {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name.as_ref().map(|name| name.as_str().to_string()))
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(conn, self.id)?;
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
                .name
                .as_ref()
                .map(|n| old_tags_name.contains(n.as_str()))
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

    pub fn update_hashtags(&self, conn: &mut Connection, tags: Vec<Hashtag>) -> Result<()> {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name.as_ref().map(|name| name.as_str().to_string()))
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(conn, self.id)?;
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
                .name
                .as_ref()
                .map(|n| old_tags_name.contains(n.as_str()))
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

    pub fn url(&self, conn: &mut Connection) -> Result<String> {
        let blog = self.get_blog(conn)?;
        Ok(format!("/~/{}/{}", blog.fqn, self.slug))
    }

    pub fn cover_url(&self, conn: &mut Connection) -> Option<String> {
        self.cover_id
            .and_then(|i| Media::get(conn, i).ok())
            .and_then(|c| c.url().ok())
    }

    pub fn build_delete(&self, conn: &mut Connection) -> Result<Delete> {
        let mut tombstone = Tombstone::new();
        tombstone.set_id(self.ap_url.parse()?);

        let mut act = Delete::new(
            self.get_authors(conn)?[0]
                .clone()
                .into_id()
                .parse::<IriString>()?,
            Base::retract(tombstone)?.into_generic()?,
        );

        act.set_id(format!("{}#delete", self.ap_url).parse()?);
        act.set_many_tos(vec![PUBLIC_VISIBILITY.parse::<IriString>()?]);
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

impl FromId<Connection> for Post {
    type Error = Error;
    type Object = LicensedArticle;

    fn from_db(conn: &mut Connection, id: &str) -> Result<Self> {
        Self::find_by_ap_url(conn, id)
    }

    async fn from_activity(conn: &mut Connection, article: LicensedArticle) -> Result<Self> {
        let license = article.ext_one.license.unwrap_or_default();
        let article = article.inner;

        let mut authors: Vec<User> = vec![];
        let mut blog: Option<Blog> = None;
        for link in article.ap_object_ref().attributed_to().ok_or(Error::MissingApProperty)? {
            if let Some(url) = link.id() {
                match User::from_id(conn, url.as_str(), None, CONFIG.proxy()).await {
                    Ok(u) => authors.push(u),
                    Err(_) => blog = Blog::from_id(conn, url.as_str(), None, CONFIG.proxy()).await.ok(),
                };
            }
        }

        let cover = match article.icon()
                .and_then(|icon| icon.iter().next() )
                .and_then(|icon| icon.to_owned().extend::<Image, ImageType>().ok().flatten() ) {
            Some(image) => Media::from_activity(conn, &image).await.ok().map(|m| m.id),
            None => None,
        };

        let title = article
            .name()
            .and_then(|name| name.to_as_string())
            .ok_or(Error::MissingApProperty)?;
        let id = AnyBase::from_extended(article.clone()) // FIXME: Don't clone
            .ok()
            .ok_or(Error::MissingApProperty)?
            .id()
            .map(|id| id.to_string());
        let ap_url = article
            .url()
            .and_then(|url| url.to_as_uri().or(id))
            .ok_or(Error::MissingApProperty)?;
        let source = article
            .source()
            .and_then(|s| {
                serde_json::to_value(s).ok().and_then(|obj| {
                    if !obj.is_object() {
                        return None;
                    }
                    obj.get("content")
                        .and_then(|content| content.as_str().map(|c| c.to_string()))
                })
            })
            .unwrap_or_default();
        let post = Post::from_db(conn, &ap_url)
            .and_then(|mut post| {
                let mut updated = false;

                let slug = Self::slug(&title);
                let content = SafeString::new(
                    &article
                        .content()
                        .and_then(|content| content.to_as_string())
                        .ok_or(Error::MissingApProperty)?,
                );
                let subtitle = article
                    .summary()
                    .and_then(|summary| summary.to_as_string())
                    .ok_or(Error::MissingApProperty)?;

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
                    post.source = source.clone();
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
                        blog_id: blog.ok_or(Error::NotFound)?.id,
                        slug: Self::slug(&title).to_string(),
                        title,
                        content: SafeString::new(
                            &article
                                .content()
                                .and_then(|content| content.to_as_string())
                                .ok_or(Error::MissingApProperty)?,
                        ),
                        published: true,
                        license,
                        // FIXME: This is wrong: with this logic, we may use the display URL as the AP ID. We need two different fields
                        ap_url,
                        creation_date: article.published().map(|published| {
                            let timestamp_secs = published.unix_timestamp();
                            let timestamp_nanos = published.unix_timestamp_nanos() - (timestamp_secs as i128) * 1000i128 * 1000i128 * 1000i128;

                            chrono::DateTime::from_timestamp(
                                timestamp_secs,
                                timestamp_nanos as u32,
                            )
                            .expect("invalid timestamp")
                            .naive_utc()
                        }),
                        subtitle: article
                            .summary()
                            .and_then(|summary| summary.to_as_string())
                            .ok_or(Error::MissingApProperty)?,
                        source,
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
        if let Some(tags) = article.tag() {
            for tag in tags.iter() {
                tag.clone()
                    .extend::<link::Mention, MentionType>() // FIXME: Don't clone
                    .map(|mention| {
                        mention.map(|m| Mention::from_activity(conn, &m, post.id, true, true))
                    })
                    .ok();

                tag.clone()
                    .extend::<Hashtag, HashtagType>() // FIXME: Don't clone
                    .map(|hashtag| {
                        hashtag.and_then(|t| {
                            let tag_name = t.name.clone()?.as_str().to_string();
                            Tag::from_activity(conn, &t, post.id, hashtags.remove(&tag_name)).ok()
                        })
                    })
                    .ok();
            }
        }

        Timeline::add_to_all_timelines(conn, &post, &Kind::Original).await?;

        Ok(post)
    }

    fn get_sender() -> &'static dyn Signer {
        Instance::get_local_instance_user().expect("Failed to get local instance user")
    }
}

impl AsObject<User, Create, &mut Connection> for Post {
    type Error = Error;
    type Output = Self;

    async fn activity(self, _conn: &mut Connection, _actor: User, _id: &str) -> Result<Self::Output> {
        // TODO: check that _actor is actually one of the author?
        Ok(self)
    }
}

impl AsObject<User, Delete, &mut Connection> for Post {
    type Error = Error;
    type Output = ();

    async fn activity(self, conn: &mut Connection, actor: User, _id: &str) -> Result<Self::Output> {
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

impl FromId<Connection> for PostUpdate {
    type Error = Error;
    type Object = LicensedArticle;

    fn from_db(_: &mut Connection, _: &str) -> Result<Self> {
        // Always fail because we always want to deserialize the AP object
        Err(Error::NotFound)
    }

    async fn from_activity(conn: &mut Connection, updated: Self::Object) -> Result<Self> {
        let mut post_update = PostUpdate {
            ap_url: updated
                .ap_object_ref()
                .id_unchecked()
                .ok_or(Error::MissingApProperty)?
                .to_string(),
            title: updated
                .ap_object_ref()
                .name()
                .and_then(|name| name.to_as_string()),
            subtitle: updated
                .ap_object_ref()
                .summary()
                .and_then(|summary| summary.to_as_string()),
            content: updated
                .ap_object_ref()
                .content()
                .and_then(|content| content.to_as_string()),
            cover: None,
            source: updated.source().and_then(|s| {
                serde_json::to_value(s).ok().and_then(|obj| {
                    if !obj.is_object() {
                        return None;
                    }
                    obj.get("content")
                        .and_then(|content| content.as_str().map(|c| c.to_string()))
                })
            }),
            license: None,
            tags: updated
                .tag()
                .and_then(|tags| serde_json::to_value(tags).ok()),
        };
        let cover = match updated.ap_object_ref().icon()
                .and_then(|img| img.iter().next())
                .and_then(|img| img.clone().extend::<Image, ImageType>().ok().flatten() ) {
            Some(img) => Media::from_activity(conn, &img).await.ok().map(|m| m.id),
            None => None,
        };
        post_update.cover = cover;
        post_update.license = updated.ext_one.license;

        Ok(post_update)
    }

    fn get_sender() -> &'static dyn Signer {
        Instance::get_local_instance_user().expect("Failed to local instance user")
    }
}

impl AsObject<User, Update, &mut Connection> for PostUpdate {
    type Error = Error;
    type Output = ();

    async fn activity(self, conn: &mut Connection, actor: User, _id: &str) -> Result<()> {
        let mut post =
            Post::from_id(conn, &self.ap_url, None, CONFIG.proxy()).await.map_err(|(_, e)| e)?;

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
                        let tag_name = t.name.as_ref().ok_or(Error::MissingApProperty)?;
                        let tag_name_str = tag_name
                            .as_xsd_string()
                            .or_else(|| tag_name.as_rdf_lang_string().map(|rls| &*rls.value))
                            .ok_or(Error::MissingApProperty)?;
                        if txt_hashtags.remove(tag_name_str) {
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
    use crate::db_conn::DbConn;
    use crate::inbox::{inbox, tests::fill_database, InboxResult};
    use crate::mentions::{Mention, NewMention};
    use crate::safe_string::SafeString;
    use crate::tests::{db, format_datetime};
    use assert_json_diff::assert_json_eq;
    use diesel::Connection;
    use serde_json::{json, to_value};

    fn prepare_activity(conn: &DbConn) -> (Post, Mention, Vec<Post>, Vec<User>, Vec<Blog>) {
        let (posts, users, blogs) = fill_database(conn);
        let post = &posts[0];
        let mentioned = &users[1];
        let mention = Mention::insert(
            conn,
            NewMention {
                mentioned_id: mentioned.id,
                post_id: Some(post.id),
                comment_id: None,
            },
        )
        .unwrap();
        (post.to_owned(), mention, posts, users, blogs)
    }

    // creates a post, get it's Create activity, delete the post,
    // "send" the Create to the inbox, and check it works
    #[test]
    fn self_federation() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, blogs) = fill_database(conn);
            let post = Post::insert(
                conn,
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
                conn,
                NewPostAuthor {
                    post_id: post.id,
                    author_id: users[0].id,
                },
            )
            .unwrap();
            let create = post.create_activity(conn).unwrap();
            post.delete(conn).unwrap();

            match inbox(conn, serde_json::to_value(create).unwrap()).unwrap() {
                InboxResult::Post(p) => {
                    assert!(p.is_author(conn, users[0].id).unwrap());
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
    fn to_activity() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (post, _mention, _posts, _users, _blogs) = prepare_activity(&conn);
            let act = post.to_activity(&conn)?;

            let expected = json!({
                "attributedTo": ["https://plu.me/@/admin/", "https://plu.me/~/BlogName/"],
                "cc": [],
                "content": "Hello",
                "id": "https://plu.me/~/BlogName/testing",
                "license": "WTFPL",
                "name": "Testing",
                "published": format_datetime(&post.creation_date),
                "source": {
                    "content": "Hello",
                    "mediaType": "text/markdown"
                },
                "summary": "Bye",
                "tag": [
                    {
                        "href": "https://plu.me/@/user/",
                        "name": "@user",
                        "type": "Mention"
                    }
                ],
                "to": ["https://www.w3.org/ns/activitystreams#Public"],
                "type": "Article",
                "url": "https://plu.me/~/BlogName/testing"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn create_activity() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (post, _mention, _posts, _users, _blogs) = prepare_activity(&conn);
            let act = post.create_activity(&conn)?;

            let expected = json!({
                "actor": "https://plu.me/@/admin/",
                "cc": [],
                "id": "https://plu.me/~/BlogName/testing/activity",
                "object": {
                    "attributedTo": ["https://plu.me/@/admin/", "https://plu.me/~/BlogName/"],
                    "cc": [],
                    "content": "Hello",
                    "id": "https://plu.me/~/BlogName/testing",
                    "license": "WTFPL",
                    "name": "Testing",
                    "published": format_datetime(&post.creation_date),
                    "source": {
                        "content": "Hello",
                        "mediaType": "text/markdown"
                    },
                    "summary": "Bye",
                    "tag": [
                        {
                            "href": "https://plu.me/@/user/",
                            "name": "@user",
                            "type": "Mention"
                        }
                    ],
                    "to": ["https://www.w3.org/ns/activitystreams#Public"],
                    "type": "Article",
                    "url": "https://plu.me/~/BlogName/testing"
                },
                "to": ["https://www.w3.org/ns/activitystreams#Public"],
                "type": "Create"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }

    #[test]
    fn update_activity() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (post, _mention, _posts, _users, _blogs) = prepare_activity(&conn);
            let act = post.update_activity(&conn)?;

            let expected = json!({
                "actor": "https://plu.me/@/admin/",
                "cc": [],
                "id": "https://plu.me/~/BlogName/testing/update-",
                "object": {
                    "attributedTo": ["https://plu.me/@/admin/", "https://plu.me/~/BlogName/"],
                    "cc": [],
                    "content": "Hello",
                    "id": "https://plu.me/~/BlogName/testing",
                    "license": "WTFPL",
                    "name": "Testing",
                    "published": format_datetime(&post.creation_date),
                    "source": {
                        "content": "Hello",
                        "mediaType": "text/markdown"
                    },
                    "summary": "Bye",
                    "tag": [
                        {
                            "href": "https://plu.me/@/user/",
                            "name": "@user",
                            "type": "Mention"
                        }
                    ],
                    "to": ["https://www.w3.org/ns/activitystreams#Public"],
                    "type": "Article",
                    "url": "https://plu.me/~/BlogName/testing"
                },
                "to": ["https://www.w3.org/ns/activitystreams#Public"],
                "type": "Update"
            });
            let actual = to_value(act)?;

            let id = actual["id"].to_string();
            let (id_pre, id_post) = id.rsplit_once('-').unwrap();
            assert_eq!(post.ap_url, "https://plu.me/~/BlogName/testing");
            assert_eq!(
                id_pre,
                to_value("\"https://plu.me/~/BlogName/testing/update")
                    .unwrap()
                    .as_str()
                    .unwrap()
            );
            assert_eq!(id_post.len(), 11);
            assert_eq!(
                id_post.matches(char::is_numeric).collect::<String>().len(),
                10
            );
            for (key, value) in actual.as_object().unwrap().into_iter() {
                if key == "id" {
                    continue;
                }
                assert_json_eq!(value, expected.get(key).unwrap());
            }

            Ok(())
        });
    }

    #[test]
    fn build_delete() {
        let conn = db();
        conn.test_transaction::<_, Error, _>(|| {
            let (post, _mention, _posts, _users, _blogs) = prepare_activity(&conn);
            let act = post.build_delete(&conn)?;

            let expected = json!({
                "actor": "https://plu.me/@/admin/",
                "id": "https://plu.me/~/BlogName/testing#delete",
                "object": {
                    "id": "https://plu.me/~/BlogName/testing",
                    "type": "Tombstone"
                },
                "to": [
                    "https://www.w3.org/ns/activitystreams#Public"
                ],
                "type": "Delete"
            });

            assert_json_eq!(to_value(act)?, expected);

            Ok(())
        });
    }
}
