use chrono::NaiveDateTime;
use rocket_contrib::json::Json;

use crate::api::{authorization::*, Api, ApiError};
use plume_api::posts::*;
use plume_api::translations::*;
use plume_common::{activity_pub::broadcast, utils::md_to_html};
use plume_models::{
    blogs::Blog, db_conn::DbConn, instance::Instance, medias::Media, mentions::*, post_authors::*,
    post_translations::*, posts::*, safe_string::SafeString, tags::*, timeline::*, translate,
    users::User, Error, PlumeRocket, CONFIG,
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
                })
            })
            .collect(),
    ))
}

#[post("/posts", data = "<payload>")]
pub fn create(
    auth: Authorization<Write, Post>,
    payload: Json<NewPostData>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Api<PostData> {
    let worker = &rockets.worker;

    let author = User::get(&conn, auth.0.user_id)?;

    let slug = Post::slug(&payload.title);
    let date = payload.creation_date.clone().and_then(|d| {
        NaiveDateTime::parse_from_str(format!("{} 00:00:00", d).as_ref(), "%Y-%m-%d %H:%M:%S").ok()
    });

    let domain = &Instance::get_local()?.public_domain;
    let (content, mentions, hashtags) = md_to_html(
        &payload.source,
        Some(domain),
        false,
        Some(Media::get_media_processor(&conn, vec![&author])),
    );

    let blog = payload
        .blog_id
        .or_else(|| {
            let blogs = Blog::find_for_author(&conn, &author).ok()?;
            if blogs.len() == 1 {
                Some(blogs[0].id)
            } else {
                None
            }
        })
        .ok_or(ApiError(Error::NotFound))?;

    // The token owner must be an author of the blog they publish to.
    if !author.is_author_in(&conn, &Blog::get(&conn, blog)?)? {
        return Err(Error::Unauthorized.into());
    }

    if Post::find_by_slug(&conn, slug, blog).is_ok() {
        return Err(Error::InvalidValue.into());
    }

    let post = Post::insert(
        &conn,
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
        &conn,
        NewPostAuthor {
            author_id: author.id,
            post_id: post.id,
        },
    )?;

    if let Some(ref tags) = payload.tags {
        for tag in tags {
            Tag::insert(
                &conn,
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
            &conn,
            NewTag {
                tag: hashtag,
                is_hashtag: true,
                post_id: post.id,
            },
        )?;
    }

    if post.published {
        for m in mentions.into_iter() {
            Mention::from_activity(
                &conn,
                &Mention::build_activity(&conn, &m)?,
                post.id,
                true,
                true,
            )?;
        }

        let act = post.create_activity(&conn)?;
        let dest = User::one_by_instance(&conn)?;
        worker.execute(move || broadcast(&author, act, dest, CONFIG.proxy().cloned()));
    }

    Timeline::add_to_all_timelines(&conn, &post, Kind::Original)?;

    Ok(Json(PostData {
        authors: post
            .get_authors(&conn)?
            .into_iter()
            .map(|a| a.fqn)
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
    }))
}

#[delete("/posts/<id>")]
pub fn delete(auth: Authorization<Write, Post>, conn: DbConn, id: i32) -> Api<()> {
    let author = User::get(&conn, auth.0.user_id)?;
    if let Ok(post) = Post::get(&conn, id) {
        if post.is_author(&conn, author.id).unwrap_or(false) {
            post.delete(&conn)?;
        }
    }
    Ok(Json(()))
}

#[post("/posts/<id>/translate", data = "<payload>")]
pub fn translate(
    id: i32,
    auth: Authorization<Write, Post>,
    payload: Json<TranslationRequest>,
    conn: DbConn,
) -> Api<TranslationData> {
    let user = User::get(&conn, auth.0.user_id)?;
    let post = Post::get(&conn, id)?;

    if !post.published && !post.is_author(&conn, user.id)? {
        return Err(Error::Unauthorized.into());
    }

    if CONFIG.libretranslate.is_none() {
        return Err(ApiError(Error::InvalidValue));
    }

    let source_lang = payload
        .source_lang
        .as_deref()
        .unwrap_or("auto");
    let target_lang = &payload.target_lang;

    if target_lang.is_empty() {
        return Err(ApiError(Error::InvalidValue));
    }

    if source_lang != "auto" && source_lang == target_lang.as_str() {
        return Err(ApiError(Error::InvalidValue));
    }

    // Check if a translation already exists
    if let Ok(existing) =
        PostTranslation::find_by_post_and_lang(&conn, post.id, target_lang)
    {
        return Ok(Json(TranslationData {
            id: existing.id,
            post_id: existing.post_id,
            source_lang: existing.source_lang,
            target_lang: existing.target_lang,
            title: existing.title,
            subtitle: existing.subtitle,
            content: existing.content,
            source: existing.source,
            creation_date: existing
                .creation_date
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
        }));
    }

    let result = translate::translate_post_fields(
        &post.title,
        &post.subtitle,
        post.content.get(),
        &post.source,
        source_lang,
        target_lang,
    )
    .map_err(|_| Error::Request)?;

    // Translating an article to the language it is already written in is a
    // no-op: don't store a useless translation.
    if result.detected_language.as_deref() == Some(target_lang.as_str()) {
        return Err(ApiError(Error::InvalidValue));
    }

    // When the source language was auto-detected, store the language reported
    // by the translation instance instead of the literal "auto".
    let stored_source_lang = if source_lang == "auto" {
        result
            .detected_language
            .clone()
            .unwrap_or_else(|| source_lang.to_string())
    } else {
        source_lang.to_string()
    };

    let translation = PostTranslation::insert(
        &conn,
        NewPostTranslation {
            post_id: post.id,
            source_lang: stored_source_lang,
            target_lang: target_lang.clone(),
            title: result.title.clone(),
            subtitle: result.subtitle.clone(),
            content: result.content.clone(),
            source: result.source.clone(),
        },
    )?;

    Ok(Json(TranslationData {
        id: translation.id,
        post_id: translation.post_id,
        source_lang: translation.source_lang,
        target_lang: translation.target_lang,
        title: result.title,
        subtitle: result.subtitle,
        content: result.content,
        source: result.source,
        creation_date: translation
            .creation_date
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
    }))
}

#[get("/translations/languages")]
pub fn languages() -> Api<Vec<translate::Language>> {
    if CONFIG.libretranslate.is_none() {
        return Err(ApiError(Error::InvalidValue));
    }
    Ok(Json(translate::supported_languages()?))
}

#[get("/posts/<id>/translations")]
pub fn list_translations(
    id: i32,
    auth: Option<Authorization<Read, Post>>,
    conn: DbConn,
) -> Api<Vec<TranslationData>> {
    let user = auth.and_then(|a| User::get(&conn, a.0.user_id).ok());
    let post = Post::get(&conn, id)?;

    if !post.published
        && !user
            .and_then(|u| post.is_author(&conn, u.id).ok())
            .unwrap_or(false)
    {
        return Err(Error::Unauthorized.into());
    }

    let translations = PostTranslation::for_post(&conn, post.id)?;
    Ok(Json(
        translations
            .into_iter()
            .map(|t| TranslationData {
                id: t.id,
                post_id: t.post_id,
                source_lang: t.source_lang,
                target_lang: t.target_lang,
                title: t.title,
                subtitle: t.subtitle,
                content: t.content,
                source: t.source,
                creation_date: t
                    .creation_date
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string(),
            })
            .collect(),
    ))
}

#[delete("/posts/<id>/translations/<lang>")]
pub fn delete_translation(
    id: i32,
    lang: String,
    auth: Authorization<Write, Post>,
    conn: DbConn,
) -> Api<()> {
    let author = User::get(&conn, auth.0.user_id)?;
    let post = Post::get(&conn, id)?;

    if !post.is_author(&conn, author.id).unwrap_or(false) {
        return Err(Error::Unauthorized.into());
    }

    if let Ok(translation) = PostTranslation::find_by_post_and_lang(&conn, post.id, &lang) {
        translation.delete(&conn)?;
    }

    Ok(Json(()))
}
