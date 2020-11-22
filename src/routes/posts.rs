use chrono::Utc;
use heck::KebabCase;
use rocket::request::LenientForm;
use rocket::response::{Flash, Redirect};
use rocket_i18n::I18n;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::Duration,
};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::routes::{
    comments::NewCommentForm, errors::ErrorPage, ContentLen, RemoteForm, RespondOrRedirect,
};
use crate::template_utils::{IntoContext, Ructe};
use plume_common::activity_pub::{broadcast, ActivityStream, ApRequest};
use plume_common::utils;
use plume_models::{
    blogs::*,
    comments::{Comment, CommentTree},
    inbox::inbox,
    instance::Instance,
    medias::Media,
    mentions::Mention,
    post_authors::*,
    posts::*,
    safe_string::SafeString,
    tags::*,
    timeline::*,
    users::User,
    Error, PlumeRocket,
};

#[get("/~/<blog>/<slug>?<responding_to>", rank = 4)]
pub fn details(
    blog: String,
    slug: String,
    responding_to: Option<i32>,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let user = rockets.user.clone();
    let blog = Blog::find_by_fqn(&rockets, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, blog.id)?;
    if !(post.published
        || post
            .get_authors(&*conn)?
            .into_iter()
            .any(|a| a.id == user.clone().map(|u| u.id).unwrap_or(0)))
    {
        return Ok(render!(errors::not_authorized(
            &rockets.to_context(),
            i18n!(rockets.intl.catalog, "This post isn't published yet.")
        )));
    }

    let comments = CommentTree::from_post(&*conn, &post, user.as_ref())?;

    let previous = responding_to.and_then(|r| Comment::get(&*conn, r).ok());

    Ok(render!(posts::details(
            &rockets.to_context(),
            post.clone(),
            blog,
            &NewCommentForm {
                warning: previous.clone().map(|p| p.spoiler_text).unwrap_or_default(),
                content: previous.clone().and_then(|p| Some(format!(
                    "@{} {}",
                    p.get_author(&*conn).ok()?.fqn,
                    Mention::list_for_comment(&*conn, p.id).ok()?
                        .into_iter()
                        .filter_map(|m| {
                            let user = user.clone();
                            if let Ok(mentioned) = m.get_mentioned(&*conn) {
                                if user.is_none() || mentioned.id != user.expect("posts::details_response: user error while listing mentions").id {
                                    Some(format!("@{}", mentioned.fqn))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }).collect::<Vec<String>>().join(" "))
                    )).unwrap_or_default(),
                ..NewCommentForm::default()
            },
            ValidationErrors::default(),
            Tag::for_post(&*conn, post.id)?,
            comments,
            previous,
            post.count_likes(&*conn)?,
            post.count_reshares(&*conn)?,
            user.clone().and_then(|u| u.has_liked(&*conn, &post).ok()).unwrap_or(false),
            user.clone().and_then(|u| u.has_reshared(&*conn, &post).ok()).unwrap_or(false),
            user.and_then(|u| u.is_following(&*conn, post.get_authors(&*conn).ok()?[0].id).ok()).unwrap_or(false),
            post.get_authors(&*conn)?[0].clone()
        )))
}

#[get("/~/<blog>/<slug>", rank = 3)]
pub fn activity_details(
    blog: String,
    slug: String,
    _ap: ApRequest,
    rockets: PlumeRocket,
) -> Result<ActivityStream<LicensedArticle>, Option<String>> {
    let conn = &*rockets.conn;
    let blog = Blog::find_by_fqn(&rockets, &blog).map_err(|_| None)?;
    let post = Post::find_by_slug(&*conn, &slug, blog.id).map_err(|_| None)?;
    if post.published {
        Ok(ActivityStream::new(
            post.to_activity(&*conn)
                .map_err(|_| String::from("Post serialization error"))?,
        ))
    } else {
        Err(Some(String::from("Not published yet.")))
    }
}

#[get("/~/<blog>/new", rank = 2)]
pub fn new_auth(blog: String, i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(
            i18n.catalog,
            "To write a new post, you need to be logged in"
        ),
        uri!(new: blog = blog),
    )
}

#[get("/~/<blog>/new", rank = 1)]
pub fn new(blog: String, cl: ContentLen, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let b = Blog::find_by_fqn(&rockets, &blog)?;
    let user = rockets.user.clone().unwrap();

    if !user.is_author_in(&*conn, &b)? {
        // TODO actually return 403 error code
        return Ok(render!(errors::not_authorized(
            &rockets.to_context(),
            i18n!(rockets.intl.catalog, "You are not an author of this blog.")
        )));
    }

    let medias = Media::for_user(&*conn, user.id)?;
    Ok(render!(posts::new(
        &rockets.to_context(),
        i18n!(rockets.intl.catalog, "New post"),
        b,
        false,
        &NewPostForm {
            license: Instance::get_local()?.default_license,
            ..NewPostForm::default()
        },
        true,
        None,
        ValidationErrors::default(),
        medias,
        cl.0
    )))
}

#[get("/~/<blog>/<slug>/edit")]
pub fn edit(
    blog: String,
    slug: String,
    cl: ContentLen,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let intl = &rockets.intl.catalog;
    let b = Blog::find_by_fqn(&rockets, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, b.id)?;
    let user = rockets.user.clone().unwrap();

    if !user.is_author_in(&*conn, &b)? {
        return Ok(render!(errors::not_authorized(
            &rockets.to_context(),
            i18n!(intl, "You are not an author of this blog.")
        )));
    }

    let source = if !post.source.is_empty() {
        post.source.clone()
    } else {
        post.content.get().clone() // fallback to HTML if the markdown was not stored
    };

    let medias = Media::for_user(&*conn, user.id)?;
    let title = post.title.clone();
    Ok(render!(posts::new(
        &rockets.to_context(),
        i18n!(intl, "Edit {0}"; &title),
        b,
        true,
        &NewPostForm {
            title: post.title.clone(),
            subtitle: post.subtitle.clone(),
            content: source,
            tags: Tag::for_post(&*conn, post.id)?
                .into_iter()
                .filter_map(|t| if !t.is_hashtag { Some(t.tag) } else { None })
                .collect::<Vec<String>>()
                .join(", "),
            license: post.license.clone(),
            draft: true,
            cover: post.cover_id,
        },
        !post.published,
        Some(post),
        ValidationErrors::default(),
        medias,
        cl.0
    )))
}

#[post("/~/<blog>/<slug>/edit", data = "<form>")]
pub fn update(
    blog: String,
    slug: String,
    cl: ContentLen,
    form: LenientForm<NewPostForm>,
    rockets: PlumeRocket,
) -> RespondOrRedirect {
    let conn = &*rockets.conn;
    let b = Blog::find_by_fqn(&rockets, &blog).expect("post::update: blog error");
    let mut post =
        Post::find_by_slug(&*conn, &slug, b.id).expect("post::update: find by slug error");
    let user = rockets.user.clone().unwrap();
    let intl = &rockets.intl.catalog;

    let new_slug = if !post.published {
        form.title.to_string().to_kebab_case()
    } else {
        post.slug.clone()
    };

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
    };

    if new_slug != slug && Post::find_by_slug(&*conn, &new_slug, b.id).is_ok() {
        errors.add(
            "title",
            ValidationError {
                code: Cow::from("existing_slug"),
                message: Some(Cow::from("A post with the same title already exists.")),
                params: HashMap::new(),
            },
        );
    }

    if errors.is_empty() {
        if !user
            .is_author_in(&*conn, &b)
            .expect("posts::update: is author in error")
        {
            // actually it's not "Ok"…
            Flash::error(
                Redirect::to(uri!(super::blogs::details: name = blog, page = _)),
                i18n!(&intl, "You are not allowed to publish on this blog."),
            )
            .into()
        } else {
            let (content, mentions, hashtags) = utils::md_to_html(
                form.content.to_string().as_ref(),
                Some(
                    &Instance::get_local()
                        .expect("posts::update: Error getting local instance")
                        .public_domain,
                ),
                false,
                Some(Media::get_media_processor(
                    &conn,
                    b.list_authors(&conn)
                        .expect("Could not get author list")
                        .iter()
                        .collect(),
                )),
            );

            // update publication date if when this article is no longer a draft
            let newly_published = if !post.published && !form.draft {
                post.published = true;
                post.creation_date = Utc::now().naive_utc();
                true
            } else {
                false
            };

            post.slug = new_slug.clone();
            post.title = form.title.clone();
            post.subtitle = form.subtitle.clone();
            post.content = SafeString::new(&content);
            post.source = form.content.clone();
            post.license = form.license.clone();
            post.cover_id = form.cover;
            post.update(&*conn, &rockets.searcher)
                .expect("post::update: update error");

            if post.published {
                post.update_mentions(
                    &conn,
                    mentions
                        .into_iter()
                        .filter_map(|m| Mention::build_activity(&rockets, &m).ok())
                        .collect(),
                )
                .expect("post::update: mentions error");
            }

            let tags = form
                .tags
                .split(',')
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<HashSet<_>>()
                .into_iter()
                .filter_map(|t| {
                    Tag::build_activity(t.to_string()).ok()
                })
                .collect::<Vec<_>>();
            post.update_tags(&conn, tags)
                .expect("post::update: tags error");

            let hashtags = hashtags
                .into_iter()
                .collect::<HashSet<_>>()
                .into_iter()
                .filter_map(|t| Tag::build_activity(t).ok())
                .collect::<Vec<_>>();
            post.update_hashtags(&conn, hashtags)
                .expect("post::update: hashtags error");

            if post.published {
                if newly_published {
                    let act = post
                        .create_activity(&conn)
                        .expect("post::update: act error");
                    let dest = User::one_by_instance(&*conn).expect("post::update: dest error");
                    rockets.worker.execute(move || broadcast(&user, act, dest));

                    Timeline::add_to_all_timelines(&rockets, &post, Kind::Original).ok();
                } else {
                    let act = post
                        .update_activity(&*conn)
                        .expect("post::update: act error");
                    let dest = User::one_by_instance(&*conn).expect("posts::update: dest error");
                    rockets.worker.execute(move || broadcast(&user, act, dest));
                }
            }

            Flash::success(
                Redirect::to(uri!(details: blog = blog, slug = new_slug, responding_to = _)),
                i18n!(intl, "Your article has been updated."),
            )
            .into()
        }
    } else {
        let medias = Media::for_user(&*conn, user.id).expect("posts:update: medias error");
        render!(posts::new(
            &rockets.to_context(),
            i18n!(intl, "Edit {0}"; &form.title),
            b,
            true,
            &*form,
            form.draft,
            Some(post),
            errors,
            medias,
            cl.0
        ))
        .into()
    }
}

#[derive(Default, FromForm, Validate)]
pub struct NewPostForm {
    #[validate(custom(function = "valid_slug", message = "Invalid title"))]
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub tags: String,
    pub license: String,
    pub draft: bool,
    pub cover: Option<i32>,
}

pub fn valid_slug(title: &str) -> Result<(), ValidationError> {
    let slug = title.to_string().to_kebab_case();
    if slug.is_empty() {
        Err(ValidationError::new("empty_slug"))
    } else if slug == "new" {
        Err(ValidationError::new("invalid_slug"))
    } else {
        Ok(())
    }
}

#[post("/~/<blog_name>/new", data = "<form>")]
pub fn create(
    blog_name: String,
    form: LenientForm<NewPostForm>,
    cl: ContentLen,
    rockets: PlumeRocket,
) -> Result<RespondOrRedirect, ErrorPage> {
    let conn = &*rockets.conn;
    let blog = Blog::find_by_fqn(&rockets, &blog_name).expect("post::create: blog error");
    let slug = form.title.to_string().to_kebab_case();
    let user = rockets.user.clone().unwrap();

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
    };
    if Post::find_by_slug(&*conn, &slug, blog.id).is_ok() {
        errors.add(
            "title",
            ValidationError {
                code: Cow::from("existing_slug"),
                message: Some(Cow::from("A post with the same title already exists.")),
                params: HashMap::new(),
            },
        );
    }

    if errors.is_empty() {
        if !user
            .is_author_in(&*conn, &blog)
            .expect("post::create: is author in error")
        {
            // actually it's not "Ok"…
            return Ok(Flash::error(
                Redirect::to(uri!(super::blogs::details: name = blog_name, page = _)),
                i18n!(
                    &rockets.intl.catalog,
                    "You are not allowed to publish on this blog."
                ),
            )
            .into());
        }

        let (content, mentions, hashtags) = utils::md_to_html(
            form.content.to_string().as_ref(),
            Some(
                &Instance::get_local()
                    .expect("post::create: local instance error")
                    .public_domain,
            ),
            false,
            Some(Media::get_media_processor(
                &conn,
                blog.list_authors(&conn)
                    .expect("Could not get author list")
                    .iter()
                    .collect(),
            )),
        );

        let post = Post::insert(
            &*conn,
            NewPost {
                blog_id: blog.id,
                slug: slug.to_string(),
                title: form.title.to_string(),
                content: SafeString::new(&content),
                published: !form.draft,
                license: form.license.clone(),
                ap_url: "".to_string(),
                creation_date: None,
                subtitle: form.subtitle.clone(),
                source: form.content.clone(),
                cover_id: form.cover,
            },
            &rockets.searcher,
        )
        .expect("post::create: post save error");

        PostAuthor::insert(
            &*conn,
            NewPostAuthor {
                post_id: post.id,
                author_id: user.id,
            },
        )
        .expect("post::create: author save error");

        let tags = form
            .tags
            .split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect::<HashSet<_>>();
        for tag in tags {
            Tag::insert(
                &*conn,
                NewTag {
                    tag: tag.to_string(),
                    is_hashtag: false,
                    post_id: post.id,
                },
            )
            .expect("post::create: tags save error");
        }
        for hashtag in hashtags {
            Tag::insert(
                &*conn,
                NewTag {
                    tag: hashtag,
                    is_hashtag: true,
                    post_id: post.id,
                },
            )
            .expect("post::create: hashtags save error");
        }

        if post.published {
            for m in mentions {
                Mention::from_activity(
                    &*conn,
                    &Mention::build_activity(&rockets, &m)
                        .expect("post::create: mention build error"),
                    post.id,
                    true,
                    true,
                )
                .expect("post::create: mention save error");
            }

            let act = post
                .create_activity(&*conn)
                .expect("posts::create: activity error");
            let dest = User::one_by_instance(&*conn).expect("posts::create: dest error");
            let worker = &rockets.worker;
            worker.execute(move || broadcast(&user, act, dest));

            Timeline::add_to_all_timelines(&rockets, &post, Kind::Original)?;
        }

        Ok(Flash::success(
            Redirect::to(uri!(details: blog = blog_name, slug = slug, responding_to = _)),
            i18n!(&rockets.intl.catalog, "Your article has been saved."),
        )
        .into())
    } else {
        let medias = Media::for_user(&*conn, user.id).expect("posts::create: medias error");
        Ok(render!(posts::new(
            &rockets.to_context(),
            i18n!(rockets.intl.catalog, "New article"),
            blog,
            false,
            &*form,
            form.draft,
            None,
            errors,
            medias,
            cl.0
        ))
        .into())
    }
}

#[post("/~/<blog_name>/<slug>/delete")]
pub fn delete(
    blog_name: String,
    slug: String,
    rockets: PlumeRocket,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    let user = rockets.user.clone().unwrap();
    let post = Blog::find_by_fqn(&rockets, &blog_name)
        .and_then(|blog| Post::find_by_slug(&*rockets.conn, &slug, blog.id));

    if let Ok(post) = post {
        if !post
            .get_authors(&*rockets.conn)?
            .into_iter()
            .any(|a| a.id == user.id)
        {
            return Ok(Flash::error(
                Redirect::to(uri!(details: blog = blog_name, slug = slug, responding_to = _)),
                i18n!(intl.catalog, "You are not allowed to delete this article."),
            ));
        }

        let dest = User::one_by_instance(&*rockets.conn)?;
        let delete_activity = post.build_delete(&*rockets.conn)?;
        inbox(
            &rockets,
            serde_json::to_value(&delete_activity).map_err(Error::from)?,
        )?;

        let user_c = user.clone();
        rockets
            .worker
            .execute(move || broadcast(&user_c, delete_activity, dest));
        let conn = rockets.conn;
        rockets
            .worker
            .execute_after(Duration::from_secs(10 * 60), move || {
                user.rotate_keypair(&*conn)
                    .expect("Failed to rotate keypair");
            });

        Ok(Flash::success(
            Redirect::to(uri!(super::blogs::details: name = blog_name, page = _)),
            i18n!(intl.catalog, "Your article has been deleted."),
        ))
    } else {
        Ok(Flash::error(Redirect::to(
            uri!(super::blogs::details: name = blog_name, page = _),
        ), i18n!(intl.catalog, "It looks like the article you tried to delete doesn't exist. Maybe it is already gone?")))
    }
}

#[get("/~/<blog_name>/<slug>/remote_interact")]
pub fn remote_interact(
    rockets: PlumeRocket,
    blog_name: String,
    slug: String,
) -> Result<Ructe, ErrorPage> {
    let target = Blog::find_by_fqn(&rockets, &blog_name)
        .and_then(|blog| Post::find_by_slug(&rockets.conn, &slug, blog.id))?;
    Ok(render!(posts::remote_interact(
        &rockets.to_context(),
        target,
        super::session::LoginForm::default(),
        ValidationErrors::default(),
        RemoteForm::default(),
        ValidationErrors::default()
    )))
}

#[post("/~/<blog_name>/<slug>/remote_interact", data = "<remote>")]
pub fn remote_interact_post(
    rockets: PlumeRocket,
    blog_name: String,
    slug: String,
    remote: LenientForm<RemoteForm>,
) -> Result<RespondOrRedirect, ErrorPage> {
    let target = Blog::find_by_fqn(&rockets, &blog_name)
        .and_then(|blog| Post::find_by_slug(&rockets.conn, &slug, blog.id))?;
    if let Some(uri) = User::fetch_remote_interact_uri(&remote.remote)
        .ok()
        .map(|uri| uri.replace("{uri}", &target.ap_url))
    {
        Ok(Redirect::to(uri).into())
    } else {
        let mut errs = ValidationErrors::new();
        errs.add("remote", ValidationError {
            code: Cow::from("invalid_remote"),
            message: Some(Cow::from(i18n!(rockets.intl.catalog, "Couldn't obtain enough information about your account. Please make sure your username is correct."))),
            params: HashMap::new(),
        });
        //could not get your remote url?
        Ok(render!(posts::remote_interact(
            &rockets.to_context(),
            target,
            super::session::LoginForm::default(),
            ValidationErrors::default(),
            remote.clone(),
            errs
        ))
        .into())
    }
}
