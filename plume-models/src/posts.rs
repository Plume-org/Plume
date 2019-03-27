use activitypub::{
    activity::{Create, Delete, Update},
    link,
    object::{Article, Image, Tombstone},
    CustomObject,
};
use canapi::{Error as ApiError, Provider};
use chrono::{NaiveDateTime, TimeZone, Utc};
use diesel::{self, BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};
use heck::{CamelCase, KebabCase};
use serde_json;
use std::collections::HashSet;

use blogs::Blog;
use instance::Instance;
use medias::Media;
use mentions::Mention;
use plume_api::posts::PostEndpoint;
use plume_common::{
    activity_pub::{
        broadcast,
        inbox::{AsObject, FromId},
        Hashtag, Id, IntoId, Licensed, Source, PUBLIC_VISIBILTY,
    },
    utils::md_to_html,
};
use post_authors::*;
use safe_string::SafeString;
use schema::posts;
use search::Searcher;
use tags::*;
use users::User;
use {ap_url, ApiResult, Connection, Error, PlumeRocket, Result, CONFIG};

pub type LicensedArticle = CustomObject<Licensed, Article>;

#[derive(Queryable, Identifiable, Clone, AsChangeset)]
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

impl Provider<PlumeRocket> for Post {
    type Data = PostEndpoint;

    fn get(rockets: &PlumeRocket, id: i32) -> ApiResult<PostEndpoint> {
        let conn = &*rockets.conn;
        if let Ok(post) = Post::get(conn, id) {
            if !post.published
                && !rockets
                    .user
                    .clone()
                    .map(|u| post.is_author(conn, u.id).unwrap_or(false))
                    .unwrap_or(false)
            {
                return Err(ApiError::Authorization(
                    "You are not authorized to access this post yet.".to_string(),
                ));
            }
            Ok(PostEndpoint {
                id: Some(post.id),
                title: Some(post.title.clone()),
                subtitle: Some(post.subtitle.clone()),
                content: Some(post.content.get().clone()),
                source: Some(post.source.clone()),
                author: Some(
                    post.get_authors(conn)
                        .map_err(|_| ApiError::NotFound("Authors not found".into()))?[0]
                        .username
                        .clone(),
                ),
                blog_id: Some(post.blog_id),
                published: Some(post.published),
                creation_date: Some(post.creation_date.format("%Y-%m-%d").to_string()),
                license: Some(post.license.clone()),
                tags: Some(
                    Tag::for_post(conn, post.id)
                        .map_err(|_| ApiError::NotFound("Tags not found".into()))?
                        .into_iter()
                        .map(|t| t.tag)
                        .collect(),
                ),
                cover_id: post.cover_id,
            })
        } else {
            Err(ApiError::NotFound("Request post was not found".to_string()))
        }
    }

    fn list(rockets: &PlumeRocket, filter: PostEndpoint) -> Vec<PostEndpoint> {
        let conn = &*rockets.conn;
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
            .get_results::<Post>(conn)
            .map(|ps| {
                ps.into_iter()
                    .filter(|p| {
                        p.published
                            || rockets
                                .user
                                .clone()
                                .map(|u| p.is_author(conn, u.id).unwrap_or(false))
                                .unwrap_or(false)
                    })
                    .map(|p| PostEndpoint {
                        id: Some(p.id),
                        title: Some(p.title.clone()),
                        subtitle: Some(p.subtitle.clone()),
                        content: Some(p.content.get().clone()),
                        source: Some(p.source.clone()),
                        author: Some(p.get_authors(conn).unwrap_or_default()[0].username.clone()),
                        blog_id: Some(p.blog_id),
                        published: Some(p.published),
                        creation_date: Some(p.creation_date.format("%Y-%m-%d").to_string()),
                        license: Some(p.license.clone()),
                        tags: Some(
                            Tag::for_post(conn, p.id)
                                .unwrap_or_else(|_| vec![])
                                .into_iter()
                                .map(|t| t.tag)
                                .collect(),
                        ),
                        cover_id: p.cover_id,
                    })
                    .collect()
            })
            .unwrap_or_else(|_| vec![])
    }

    fn update(
        _rockets: &PlumeRocket,
        _id: i32,
        _new_data: PostEndpoint,
    ) -> ApiResult<PostEndpoint> {
        unimplemented!()
    }

    fn delete(rockets: &PlumeRocket, id: i32) {
        let conn = &*rockets.conn;
        let user_id = rockets
            .user
            .clone()
            .expect("Post as Provider::delete: not authenticated")
            .id;
        if let Ok(post) = Post::get(conn, id) {
            if post.is_author(conn, user_id).unwrap_or(false) {
                post.delete(conn, &rockets.searcher)
                    .expect("Post as Provider::delete: delete error");
            }
        }
    }

    fn create(rockets: &PlumeRocket, query: PostEndpoint) -> ApiResult<PostEndpoint> {
        let conn = &*rockets.conn;
        let search = &rockets.searcher;
        let worker = &rockets.worker;
        if rockets.user.is_none() {
            return Err(ApiError::Authorization(
                "You are not authorized to create new articles.".to_string(),
            ));
        }

        let title = query.title.clone().expect("No title for new post in API");
        let slug = query.title.unwrap().to_kebab_case();

        let date = query.creation_date.clone().and_then(|d| {
            NaiveDateTime::parse_from_str(format!("{} 00:00:00", d).as_ref(), "%Y-%m-%d %H:%M:%S")
                .ok()
        });

        let domain = &Instance::get_local(&conn)
            .map_err(|_| ApiError::NotFound("posts::update: Error getting local instance".into()))?
            .public_domain;
        let (content, mentions, hashtags) = md_to_html(
            query.source.clone().unwrap_or_default().clone().as_ref(),
            domain,
            false,
        );

        let author = rockets
            .user
            .clone()
            .ok_or_else(|| ApiError::NotFound("Author not found".into()))?;
        let blog = match query.blog_id {
            Some(x) => x,
            None => {
                Blog::find_for_author(conn, &author)
                    .map_err(|_| ApiError::NotFound("No default blog".into()))?[0]
                    .id
            }
        };

        if Post::find_by_slug(conn, &slug, blog).is_ok() {
            // Not an actual authorization problem, but we have nothing better for now…
            // TODO: add another error variant to canapi and add it there
            return Err(ApiError::Authorization(
                "A post with the same slug already exists".to_string(),
            ));
        }

        let post = Post::insert(
            conn,
            NewPost {
                blog_id: blog,
                slug,
                title,
                content: SafeString::new(content.as_ref()),
                published: query.published.unwrap_or(true),
                license: query.license.unwrap_or_else(|| {
                    Instance::get_local(conn)
                        .map(|i| i.default_license)
                        .unwrap_or_else(|_| String::from("CC-BY-SA"))
                }),
                creation_date: date,
                ap_url: String::new(),
                subtitle: query.subtitle.unwrap_or_default(),
                source: query.source.expect("Post API::create: no source error"),
                cover_id: query.cover_id,
            },
            search,
        )
        .map_err(|_| ApiError::NotFound("Creation error".into()))?;

        PostAuthor::insert(
            conn,
            NewPostAuthor {
                author_id: author.id,
                post_id: post.id,
            },
        )
        .map_err(|_| ApiError::NotFound("Error saving authors".into()))?;

        if let Some(tags) = query.tags {
            for tag in tags {
                Tag::insert(
                    conn,
                    NewTag {
                        tag,
                        is_hashtag: false,
                        post_id: post.id,
                    },
                )
                .map_err(|_| ApiError::NotFound("Error saving tags".into()))?;
            }
        }
        for hashtag in hashtags {
            Tag::insert(
                conn,
                NewTag {
                    tag: hashtag.to_camel_case(),
                    is_hashtag: true,
                    post_id: post.id,
                },
            )
            .map_err(|_| ApiError::NotFound("Error saving hashtags".into()))?;
        }

        if post.published {
            for m in mentions.into_iter() {
                Mention::from_activity(
                    &*conn,
                    &Mention::build_activity(&rockets, &m)
                        .map_err(|_| ApiError::NotFound("Couldn't build mentions".into()))?,
                    post.id,
                    true,
                    true,
                )
                .map_err(|_| ApiError::NotFound("Error saving mentions".into()))?;
            }

            let act = post
                .create_activity(&*conn)
                .map_err(|_| ApiError::NotFound("Couldn't create activity".into()))?;
            let dest = User::one_by_instance(&*conn)
                .map_err(|_| ApiError::NotFound("Couldn't list remote instances".into()))?;
            worker.execute(move || broadcast(&author, act, dest));
        }

        Ok(PostEndpoint {
            id: Some(post.id),
            title: Some(post.title.clone()),
            subtitle: Some(post.subtitle.clone()),
            content: Some(post.content.get().clone()),
            source: Some(post.source.clone()),
            author: Some(
                post.get_authors(conn)
                    .map_err(|_| ApiError::NotFound("No authors".into()))?[0]
                    .username
                    .clone(),
            ),
            blog_id: Some(post.blog_id),
            published: Some(post.published),
            creation_date: Some(post.creation_date.format("%Y-%m-%d").to_string()),
            license: Some(post.license.clone()),
            tags: Some(
                Tag::for_post(conn, post.id)
                    .map_err(|_| ApiError::NotFound("Tags not found".into()))?
                    .into_iter()
                    .map(|t| t.tag)
                    .collect(),
            ),
            cover_id: post.cover_id,
        })
    }
}

impl Post {
    get!(posts);
    find_by!(posts, find_by_slug, slug as &str, blog_id as i32);
    find_by!(posts, find_by_ap_url, ap_url as &str);

    last!(posts);
    pub fn insert(conn: &Connection, new: NewPost, searcher: &Searcher) -> Result<Self> {
        diesel::insert_into(posts::table)
            .values(new)
            .execute(conn)?;
        let mut post = Self::last(conn)?;
        if post.ap_url.is_empty() {
            post.ap_url = ap_url(&format!(
                "{}/~/{}/{}/",
                CONFIG.base_url,
                post.get_blog(conn)?.fqn,
                post.slug
            ));
            let _: Post = post.save_changes(conn)?;
        }

        searcher.add_document(conn, &post)?;
        Ok(post)
    }

    pub fn update(&self, conn: &Connection, searcher: &Searcher) -> Result<Self> {
        diesel::update(self).set(self).execute(conn)?;
        let post = Self::get(conn, self.id)?;
        searcher.update_document(conn, &post)?;
        Ok(post)
    }

    pub fn delete(&self, conn: &Connection, searcher: &Searcher) -> Result<()> {
        for m in Mention::list_for_post(&conn, self.id)? {
            m.delete(conn)?;
        }
        diesel::delete(self).execute(conn)?;
        searcher.delete_document(self);
        Ok(())
    }

    pub fn list_by_tag(
        conn: &Connection,
        tag: String,
        (min, max): (i32, i32),
    ) -> Result<Vec<Post>> {
        use schema::tags;

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
        use schema::tags;
        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .count()
            .load(conn)?
            .iter()
            .next()
            .cloned()
            .ok_or(Error::NotFound)
    }

    pub fn count_local(conn: &Connection) -> Result<i64> {
        use schema::post_authors;
        use schema::users;
        let local_authors = users::table
            .filter(users::instance_id.eq(Instance::get_local(conn)?.id))
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

    pub fn get_recents(conn: &Connection, limit: i64) -> Result<Vec<Post>> {
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .limit(limit)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_recents_for_author(
        conn: &Connection,
        author: &User,
        limit: i64,
    ) -> Result<Vec<Post>> {
        use schema::post_authors;

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

    /// Give a page of all the recent posts known to this instance (= federated timeline)
    pub fn get_recents_page(conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<Post>> {
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    /// Give a page of posts from a specific instance
    pub fn get_instance_page(
        conn: &Connection,
        instance_id: i32,
        (min, max): (i32, i32),
    ) -> Result<Vec<Post>> {
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
            .map_err(Error::from)
    }

    /// Give a page of customized user feed, based on a list of followed users
    pub fn user_feed_page(
        conn: &Connection,
        followed: Vec<i32>,
        (min, max): (i32, i32),
    ) -> Result<Vec<Post>> {
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
            .map_err(Error::from)
    }

    pub fn drafts_by_author(conn: &Connection, author: &User) -> Result<Vec<Post>> {
        use schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(false))
            .filter(posts::id.eq_any(posts))
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_authors(&self, conn: &Connection) -> Result<Vec<User>> {
        use schema::post_authors;
        use schema::users;
        let author_list = PostAuthor::belonging_to(self).select(post_authors::author_id);
        users::table
            .filter(users::id.eq_any(author_list))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn is_author(&self, conn: &Connection, author_id: i32) -> Result<bool> {
        use schema::post_authors;
        Ok(PostAuthor::belonging_to(self)
            .filter(post_authors::author_id.eq(author_id))
            .count()
            .get_result::<i64>(conn)?
            > 0)
    }

    pub fn get_blog(&self, conn: &Connection) -> Result<Blog> {
        use schema::blogs;
        blogs::table
            .filter(blogs::id.eq(self.blog_id))
            .limit(1)
            .load::<Blog>(conn)?
            .into_iter()
            .nth(0)
            .ok_or(Error::NotFound)
    }

    pub fn count_likes(&self, conn: &Connection) -> Result<i64> {
        use schema::likes;
        likes::table
            .filter(likes::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn count_reshares(&self, conn: &Connection) -> Result<i64> {
        use schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_receivers_urls(&self, conn: &Connection) -> Result<Vec<String>> {
        let followers = self
            .get_authors(conn)?
            .into_iter()
            .filter_map(|a| a.get_followers(conn).ok())
            .collect::<Vec<Vec<User>>>();
        Ok(followers.into_iter().fold(vec![], |mut acc, f| {
            for x in f {
                acc.push(x.ap_url);
            }
            acc
        }))
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<LicensedArticle> {
        let cc = self.get_receivers_urls(conn)?;
        let to = vec![PUBLIC_VISIBILTY.to_string()];

        let mut mentions_json = Mention::list_for_post(conn, self.id)?
            .into_iter()
            .map(|m| json!(m.to_activity(conn).ok()))
            .collect::<Vec<serde_json::Value>>();
        let mut tags_json = Tag::for_post(conn, self.id)?
            .into_iter()
            .map(|t| json!(t.to_activity(conn).ok()))
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
            cover.object_props.set_url_string(media.url(conn)?)?;
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
            .filter_map(|(id, m)| {
                if let Some(id) = id {
                    Some((m, id))
                } else {
                    None
                }
            })
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
            .and_then(|c| c.url(conn).ok())
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
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILTY)])?;
        Ok(act)
    }
}

impl FromId<PlumeRocket> for Post {
    type Error = Error;
    type Object = LicensedArticle;

    fn from_db(c: &PlumeRocket, id: &str) -> Result<Self> {
        Self::find_by_ap_url(&c.conn, id)
    }

    fn from_activity(c: &PlumeRocket, article: LicensedArticle) -> Result<Self> {
        let conn = &*c.conn;
        let searcher = &c.searcher;
        let license = article.custom_props.license_string().unwrap_or_default();
        let article = article.object;

        let (blog, authors) = article
            .object_props
            .attributed_to_link_vec::<Id>()?
            .into_iter()
            .fold((None, vec![]), |(blog, mut authors), link| {
                let url: String = link.into();
                match User::from_id(&c, &url, None) {
                    Ok(u) => {
                        authors.push(u);
                        (blog, authors)
                    }
                    Err(_) => (blog.or_else(|| Blog::from_id(&c, &url, None).ok()), authors),
                }
            });

        let cover = article
            .object_props
            .icon_object::<Image>()
            .ok()
            .and_then(|img| Media::from_activity(&c, &img).ok().map(|m| m.id));

        let title = article.object_props.name_string()?;
        let post = Post::insert(
            conn,
            NewPost {
                blog_id: blog?.id,
                slug: title.to_kebab_case(),
                title,
                content: SafeString::new(&article.object_props.content_string()?),
                published: true,
                license,
                // FIXME: This is wrong: with this logic, we may use the display URL as the AP ID. We need two different fields
                ap_url: article
                    .object_props
                    .url_string()
                    .or_else(|_| article.object_props.id_string())?,
                creation_date: Some(article.object_props.published_utctime()?.naive_utc()),
                subtitle: article.object_props.summary_string()?,
                source: article.ap_object_props.source_object::<Source>()?.content,
                cover_id: cover,
            },
            searcher,
        )?;

        for author in authors {
            PostAuthor::insert(
                conn,
                NewPostAuthor {
                    post_id: post.id,
                    author_id: author.id,
                },
            )?;
        }

        // save mentions and tags
        let mut hashtags = md_to_html(&post.source, "", false)
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
        Ok(post)
    }
}

impl AsObject<User, Create, &PlumeRocket> for Post {
    type Error = Error;
    type Output = Post;

    fn activity(self, _c: &PlumeRocket, _actor: User, _id: &str) -> Result<Post> {
        // TODO: check that _actor is actually one of the author?
        Ok(self)
    }
}

impl AsObject<User, Delete, &PlumeRocket> for Post {
    type Error = Error;
    type Output = ();

    fn activity(self, c: &PlumeRocket, actor: User, _id: &str) -> Result<()> {
        let can_delete = self
            .get_authors(&c.conn)?
            .into_iter()
            .any(|a| actor.id == a.id);
        if can_delete {
            self.delete(&c.conn, &c.searcher).map(|_| ())
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
    pub source: Option<String>,
    pub license: Option<String>,
    pub tags: Option<serde_json::Value>,
}

impl FromId<PlumeRocket> for PostUpdate {
    type Error = Error;
    type Object = LicensedArticle;

    fn from_db(_: &PlumeRocket, _: &str) -> Result<Self> {
        // Always fail because we always want to deserialize the AP object
        Err(Error::NotFound)
    }

    fn from_activity(_c: &PlumeRocket, updated: LicensedArticle) -> Result<Self> {
        Ok(PostUpdate {
            ap_url: updated.object.object_props.id_string()?,
            title: updated.object.object_props.name_string().ok(),
            subtitle: updated.object.object_props.summary_string().ok(),
            content: updated.object.object_props.content_string().ok(),
            source: updated
                .object
                .ap_object_props
                .source_object::<Source>()
                .ok()
                .map(|x| x.content),
            license: updated.custom_props.license_string().ok(),
            tags: updated.object.object_props.tag.clone(),
        })
    }
}

impl AsObject<User, Update, &PlumeRocket> for PostUpdate {
    type Error = Error;
    type Output = ();

    fn activity(self, c: &PlumeRocket, actor: User, _id: &str) -> Result<()> {
        let conn = &*c.conn;
        let searcher = &c.searcher;
        let mut post = Post::from_id(c, &self.ap_url, None)?;

        if !post.is_author(conn, actor.id)? {
            // TODO: maybe the author was added in the meantime
            return Err(Error::Unauthorized);
        }

        if let Some(title) = self.title {
            post.slug = title.to_kebab_case();
            post.title = title;
        }

        if let Some(content) = self.content {
            post.content = SafeString::new(&content);
        }

        if let Some(subtitle) = self.subtitle {
            post.subtitle = subtitle;
        }

        if let Some(source) = self.source {
            post.source = source;
        }

        if let Some(license) = self.license {
            post.license = license;
        }

        let mut txt_hashtags = md_to_html(&post.source, "", false)
            .2
            .into_iter()
            .map(|s| s.to_camel_case())
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

        post.update(conn, searcher)?;
        Ok(())
    }
}

impl IntoId for Post {
    fn into_id(self) -> Id {
        Id::new(self.ap_url.clone())
    }
}

#[cfg(test)]
mod tests {
    use diesel::Connection;
    use super::*;
    use crate::safe_string::SafeString;
    use crate::tests::rockets;
    use crate::inbox::{inbox, InboxResult, tests::fill_database};

    // creates a post, get it's Create activity, delete the post,
    // "send" the Create to the inbox, and check it works
    #[test]
    fn self_federation() {
        let r = rockets();
        let conn = &*r.conn;
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, blogs) = fill_database(&r);
            let post = Post::insert(conn, NewPost {
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
            }, &r.searcher).unwrap();
            PostAuthor::insert(conn, NewPostAuthor {
                post_id: post.id,
                author_id: users[0].id,
            }).unwrap();
            let create = post.create_activity(conn).unwrap();
            post.delete(conn, &r.searcher).unwrap();

            match inbox(&r, serde_json::to_value(create).unwrap()).unwrap() {
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
    fn licensed_article_serde() {
        let mut article = Article::default();
        article.object_props.set_id_string("Yo".into()).unwrap();
        let mut license = Licensed::default();
        license.set_license_string("WTFPL".into()).unwrap();
        let full_article = LicensedArticle::new(article, license);

        let json = serde_json::to_value(full_article).unwrap();
        let article_from_json: LicensedArticle = serde_json::from_value(json).unwrap();
        assert_eq!("Yo", &article_from_json.object.object_props.id_string().unwrap());
        assert_eq!("WTFPL", &article_from_json.custom_props.license_string().unwrap());
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
            "to": [plume_common::activity_pub::PUBLIC_VISIBILTY]
        });
        let article: LicensedArticle = serde_json::from_value(json).unwrap();
        assert_eq!("https://plu.me/~/Blog/my-article", &article.object.object_props.id_string().unwrap());
    }
}