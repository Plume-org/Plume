use chrono::NaiveDateTime;
use rocket::serde::json::Json;

use crate::api::{authorization::*, Api, ApiError};
use plume_api::posts::*;
use plume_common::{activity_pub::broadcast, utils::md_to_html};
use plume_models::{
    blogs::Blog, db_conn::DbConn, instance::Instance, medias::Media, mentions::*, post_authors::*,
    posts::*, safe_string::SafeString, tags::*, timeline::*, users::User, Error, PlumeRocket,
    CONFIG,
};

#[get("/posts/<id>")]
pub fn get(id: i32, auth: Option<Authorization<Read, Post>>, mut conn: DbConn) -> Api<PostData> {
    let user = auth.and_then(|a| User::get(&mut conn, a.0.user_id).ok());
    let post = Post::get(&mut conn, id)?;

    if !post.published
        && !user
            .and_then(|u| post.is_author(&mut conn, u.id).ok())
            .unwrap_or(false)
    {
        return Err(Error::Unauthorized.into());
    }

    Ok(Json(PostData {
        authors: post
            .get_authors(&mut conn)?
            .into_iter()
            .map(|a| a.username)
            .collect(),
        creation_date: post.creation_date.format("%Y-%m-%d").to_string(),
        tags: Tag::for_post(&mut conn, post.id)?
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
    }))
}

#[get("/posts?<title>&<subtitle>&<content>")]
pub fn list(
    title: Option<String>,
    subtitle: Option<String>,
    content: Option<String>,
    auth: Option<Authorization<Read, Post>>,
    mut conn: DbConn,
) -> Api<Vec<PostData>> {
    let user = auth.and_then(|a| User::get(&mut conn, a.0.user_id).ok());
    let user_id = user.map(|u| u.id);

    Ok(Json(
        Post::list_filtered(&mut conn, title, subtitle, content)?
            .into_iter()
            .filter_map(|p| {
                if p.published || user_id
                    .and_then(|u| p.is_author(&mut conn, u).ok())
                    .unwrap_or(false) {

                    let authors = p
                        .get_authors(&mut conn)
                        .ok()?
                        .into_iter()
                        .map(|a| a.username)
                        .collect();

                    let tags = Tag::for_post(&mut conn, p.id)
                        .ok()?
                        .into_iter()
                        .map(|t| t.tag)
                        .collect();

                    Some(PostData {
                        authors,
                        creation_date: p.creation_date.format("%Y-%m-%d").to_string(),
                        tags,
                        id: p.id,
                        title: p.title,
                        subtitle: p.subtitle,
                        content: p.content.to_string(),
                        source: Some(p.source),
                        blog_id: p.blog_id,
                        published: p.published,
                        license: p.license,
                        cover_id: p.cover_id,
                    })
                } else {
                    None
                }
            })
            .collect(),
    ))
}

#[post("/posts", data = "<payload>")]
pub async fn create(
    auth: Authorization<Write, Post>,
    payload: Json<NewPostData>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Api<PostData> {
    let worker = &rockets.worker;

    let author = User::get(&mut conn, auth.0.user_id)?;

    let slug = Post::slug(&payload.title);
    let date = payload.creation_date.clone().and_then(|d| {
        NaiveDateTime::parse_from_str(format!("{} 00:00:00", d).as_ref(), "%Y-%m-%d %H:%M:%S").ok()
    });

    let domain = &Instance::get_local()?.public_domain;
    let (content, mentions, hashtags) = md_to_html(
        &payload.source,
        Some(domain),
        false,
        Some(Media::get_media_processor(&mut conn, vec![&author])),
    );

    let blog = payload
        .blog_id
        .or_else(|| {
            let blogs = Blog::find_for_author(&mut conn, &author).ok()?;
            if blogs.len() == 1 {
                Some(blogs[0].id)
            } else {
                None
            }
        })
        .ok_or(ApiError(Error::NotFound))?;

    if Post::find_by_slug(&mut conn, slug, blog).is_ok() {
        return Err(Error::InvalidValue.into());
    }

    let post = Post::insert(
        &mut conn,
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
    )?;

    PostAuthor::insert(
        &mut conn,
        NewPostAuthor {
            author_id: author.id,
            post_id: post.id,
        },
    )?;

    if let Some(ref tags) = payload.tags {
        for tag in tags {
            Tag::insert(
                &mut conn,
                NewTag {
                    tag: tag.to_string(),
                    is_hashtag: false,
                    post_id: post.id,
                },
            )?;
        }
    }
    for hashtag in hashtags {
        Tag::insert(
            &mut conn,
            NewTag {
                tag: hashtag,
                is_hashtag: true,
                post_id: post.id,
            },
        )?;
    }

    if post.published {
        for m in mentions.into_iter() {
            let activity = &Mention::build_activity(&mut conn, &m).await?;
            Mention::from_activity(
                &mut conn,
                activity,
                post.id,
                true,
                true,
            )?;
        }

        let act = post.create_activity(&mut conn)?;
        let dest = User::one_by_instance(&mut conn)?;
        worker.execute(move || broadcast(&author, act, dest, CONFIG.proxy().cloned()));
    }

    Timeline::add_to_all_timelines(&mut conn, &post, &Kind::Original).await?;

    Ok(Json(PostData {
        authors: post
            .get_authors(&mut conn)?
            .into_iter()
            .map(|a| a.fqn)
            .collect(),
        creation_date: post.creation_date.format("%Y-%m-%d").to_string(),
        tags: Tag::for_post(&mut conn, post.id)?
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
    }))
}

#[delete("/posts/<id>")]
pub fn delete(auth: Authorization<Write, Post>, mut conn: DbConn, id: i32) -> Api<()> {
    let author = User::get(&mut conn, auth.0.user_id)?;
    if let Ok(post) = Post::get(&mut conn, id) {
        if post.is_author(&mut conn, author.id).unwrap_or(false) {
            post.delete(&mut conn)?;
        }
    }
    Ok(Json(()))
}
