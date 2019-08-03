use chrono::{NaiveDateTime, Utc};
use heck::{CamelCase, KebabCase};
use rocket_contrib::json::Json;
use std::collections::HashSet;

use crate::api::{authorization::*, Api};
use plume_api::posts::*;
use plume_common::{activity_pub::broadcast, utils::md_to_html};
use plume_models::{
    blogs::Blog, db_conn::DbConn, instance::Instance, medias::Media, mentions::*, post_authors::*,
    posts::*, safe_string::SafeString, tags::*, users::User, Error, PlumeRocket,
};

#[get("/posts/<id>")]
pub fn get(id: i32, auth: Option<Authorization<Read, Post>>, conn: DbConn) -> Api<PostData> {
    let user = auth.and_then(|a| User::get(&conn, a.0.user_id).ok());
    let post = Post::get(&conn, id)?;

    if !post.published
        && !user
            .and_then(|u| post.is_author(&conn, u.id).ok())
            .unwrap_or(false)
    {
        return Err(Error::Unauthorized.into());
    }

    Ok(Json(PostData {
        authors: post
            .get_authors(&conn)?
            .into_iter()
            .map(|a| a.username)
            .collect(),
        creation_date: post.creation_date.format("%Y-%m-%d").to_string(),
        tags: Tag::for_post(&conn, post.id)?
            .into_iter()
            .map(|t| t.tag)
            .collect(),

        id: post.id,
        title: post.title,
        subtitle: post.subtitle,
        content: post.content.to_string(),
        source: Some(post.source),
        blog_id: post.blog_id,
        published: post.published,
        license: post.license,
        cover_id: post.cover_id,
        url: post.ap_url,
    }))
}

#[get("/posts?<title>&<subtitle>&<content>")]
pub fn list(
    title: Option<String>,
    subtitle: Option<String>,
    content: Option<String>,
    auth: Option<Authorization<Read, Post>>,
    conn: DbConn,
) -> Api<Vec<PostData>> {
    let user = auth.and_then(|a| User::get(&conn, a.0.user_id).ok());
    let user_id = user.map(|u| u.id);

    Ok(Json(
        Post::list_filtered(&conn, title, subtitle, content)?
            .into_iter()
            .filter(|p| {
                p.published
                    || user_id
                        .and_then(|u| p.is_author(&conn, u).ok())
                        .unwrap_or(false)
            })
            .filter_map(|p| {
                Some(PostData {
                    authors: p
                        .get_authors(&conn)
                        .ok()?
                        .into_iter()
                        .map(|a| a.username)
                        .collect(),
                    creation_date: p.creation_date.format("%Y-%m-%d").to_string(),
                    tags: Tag::for_post(&conn, p.id)
                        .ok()?
                        .into_iter()
                        .map(|t| t.tag)
                        .collect(),

                    id: p.id,
                    title: p.title,
                    subtitle: p.subtitle,
                    content: p.content.to_string(),
                    source: Some(p.source),
                    blog_id: p.blog_id,
                    published: p.published,
                    license: p.license,
                    cover_id: p.cover_id,
                    url: p.ap_url,
                })
            })
            .collect(),
    ))
}

#[post("/posts", data = "<payload>")]
pub fn create(
    auth: Authorization<Write, Post>,
    payload: Json<NewPostData>,
    rockets: PlumeRocket,
) -> Api<PostData> {
    let conn = &*rockets.conn;
    let search = &rockets.searcher;
    let worker = &rockets.worker;

    let author = User::get(conn, auth.0.user_id)?;

    let slug = &payload.title.clone().to_kebab_case();
    let date = payload.creation_date.clone().and_then(|d| {
        NaiveDateTime::parse_from_str(format!("{} 00:00:00", d).as_ref(), "%Y-%m-%d %H:%M:%S").ok()
    });

    if slug.as_str() == "new" {
        return Err(
            Error::Validation("Sorry, but your article can't have this title.".into()).into(),
        );
    }

    if payload.title.is_empty() {
        return Err(Error::Validation("You have to give your article a title.".into()).into());
    }

    if payload.source.is_empty() {
        return Err(Error::Validation("Your article can't be empty.".into()).into());
    }

    let domain = &Instance::get_local()?.public_domain;
    let (content, mentions, hashtags) = md_to_html(
        &payload.source,
        Some(domain),
        false,
        Some(Media::get_media_processor(conn, vec![&author])),
    );

    let blog = payload.blog_id.or_else(|| {
        let blogs = Blog::find_for_author(conn, &author).ok()?;
        if blogs.len() == 1 {
            Some(blogs[0].id)
        } else {
            None
        }
    })?;

    if !author.is_author_in(conn, &Blog::get(conn, blog)?)? {
        return Err(Error::Unauthorized.into());
    }

    if Post::find_by_slug(conn, slug, blog).is_ok() {
        return Err(Error::InvalidValue.into());
    }

    let post = Post::insert(
        conn,
        NewPost {
            blog_id: blog,
            slug: slug.to_string(),
            title: payload.title.clone(),
            content: SafeString::new(content.as_ref()),
            published: payload.published.unwrap_or(true),
            license: payload.license.clone().unwrap_or_else(|| {
                Instance::get_local()
                    .map(|i| i.default_license)
                    .unwrap_or_else(|_| String::from("CC-BY-SA"))
            }),
            creation_date: date,
            ap_url: String::new(),
            subtitle: payload.subtitle.clone().unwrap_or_default(),
            source: payload.source.clone(),
            cover_id: payload.cover_id,
        },
        search,
    )?;

    PostAuthor::insert(
        conn,
        NewPostAuthor {
            author_id: author.id,
            post_id: post.id,
        },
    )?;

    if let Some(ref tags) = payload.tags {
        let tags = tags
            .iter()
            .map(|t| t.to_camel_case())
            .filter(|t| !t.is_empty())
            .collect::<HashSet<_>>()
            .into_iter()
            .filter_map(|t| Tag::build_activity(t).ok());

        for tag in tags {
            Tag::insert(
                conn,
                NewTag {
                    tag: tag.name_string().unwrap(),
                    is_hashtag: false,
                    post_id: post.id,
                },
            )?;
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
        )?;
    }

    if post.published {
        for m in mentions.into_iter() {
            Mention::from_activity(
                &*conn,
                &Mention::build_activity(&rockets, &m)?,
                post.id,
                true,
                true,
            )?;
        }

        let act = post.create_activity(&*conn)?;
        let dest = User::one_by_instance(&*conn)?;
        worker.execute(move || broadcast(&author, act, dest));
    }

    Ok(Json(PostData {
        authors: post.get_authors(conn)?.into_iter().map(|a| a.fqn).collect(),
        creation_date: post.creation_date.format("%Y-%m-%d").to_string(),
        tags: Tag::for_post(conn, post.id)?
            .into_iter()
            .map(|t| t.tag)
            .collect(),
        id: post.id,
        title: post.title,
        subtitle: post.subtitle,
        content: post.content.to_string(),
        source: Some(post.source),
        blog_id: post.blog_id,
        published: post.published,
        license: post.license,
        cover_id: post.cover_id,
        url: post.ap_url,
    }))
}

#[put("/posts/<id>", data = "<payload>")]
pub fn update(
    id: i32,
    auth: Authorization<Write, Post>,
    payload: Json<NewPostData>,
    rockets: PlumeRocket,
) -> Api<PostData> {
    let conn = &*rockets.conn;
    let mut post = Post::get(&*conn, id)?;
    let author = User::get(conn, auth.0.user_id)?;
    let b = post.get_blog(&*conn)?;

    let new_slug = if !post.published {
        payload.title.to_string().to_kebab_case()
    } else {
        post.slug.clone()
    };

    if new_slug != post.slug && Post::find_by_slug(&*conn, &new_slug, b.id).is_ok() {
        return Err(Error::Validation("A post with the same title already exists.".into()).into());
    }

    if !author.is_author_in(&*conn, &b)? {
        Err(Error::Unauthorized.into())
    } else {
        let (content, mentions, hashtags) = md_to_html(
            &payload.source,
            Some(&Instance::get_local()?.public_domain),
            false,
            Some(Media::get_media_processor(
                &conn,
                b.list_authors(&conn)?.iter().collect(),
            )),
        );

        // update publication date if when this article is no longer a draft
        let newly_published = if !post.published && payload.published.unwrap_or(post.published) {
            post.published = true;
            post.creation_date = Utc::now().naive_utc();
            true
        } else {
            false
        };

        post.slug = new_slug.clone();
        post.title = payload.title.clone();
        post.subtitle = payload.subtitle.clone().unwrap_or_default();
        post.content = SafeString::new(&content);
        post.source = payload.source.clone();
        post.license = payload.license.clone().unwrap_or_default();
        post.cover_id = payload.cover_id;
        post.update(&*conn, &rockets.searcher)?;

        if post.published {
            post.update_mentions(
                &conn,
                mentions
                    .into_iter()
                    .filter_map(|m| Mention::build_activity(&rockets, &m).ok())
                    .collect(),
            )?;
        }

        let tags = payload
            .tags
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|t| t.trim().to_camel_case())
            .filter(|t| !t.is_empty())
            .collect::<HashSet<_>>()
            .into_iter()
            .filter_map(|t| Tag::build_activity(t).ok())
            .collect::<Vec<_>>();
        post.update_tags(&conn, tags)?;

        let hashtags = hashtags
            .into_iter()
            .map(|h| h.to_camel_case())
            .collect::<HashSet<_>>()
            .into_iter()
            .filter_map(|t| Tag::build_activity(t).ok())
            .collect::<Vec<_>>();
        post.update_hashtags(&conn, hashtags)?;

        if post.published {
            if newly_published {
                let act = post.create_activity(&conn)?;
                let dest = User::one_by_instance(&*conn)?;
                rockets
                    .worker
                    .execute(move || broadcast(&author, act, dest));
            } else {
                let act = post.update_activity(&*conn)?;
                let dest = User::one_by_instance(&*conn)?;
                rockets
                    .worker
                    .execute(move || broadcast(&author, act, dest));
            }
        }

        Ok(Json(PostData {
            authors: post.get_authors(conn)?.into_iter().map(|a| a.fqn).collect(),
            creation_date: post.creation_date.format("%Y-%m-%d").to_string(),
            tags: Tag::for_post(conn, post.id)?
                .into_iter()
                .map(|t| t.tag)
                .collect(),
            id: post.id,
            title: post.title,
            subtitle: post.subtitle,
            content: post.content.to_string(),
            source: Some(post.source),
            blog_id: post.blog_id,
            published: post.published,
            license: post.license,
            cover_id: post.cover_id,
            url: post.ap_url,
        }))
    }
}

#[delete("/posts/<id>")]
pub fn delete(auth: Authorization<Write, Post>, rockets: PlumeRocket, id: i32) -> Api<()> {
    let author = User::get(&*rockets.conn, auth.0.user_id)?;
    if let Ok(post) = Post::get(&*rockets.conn, id) {
        if post.is_author(&*rockets.conn, author.id).unwrap_or(false) {
            post.delete(&*rockets.conn, &rockets.searcher)?;
        }
    }
    Ok(Json(()))
}
