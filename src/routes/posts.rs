use activitypub::activity::Delete;
use chrono::Utc;
use heck::{CamelCase, KebabCase};
use rocket::request::LenientForm;
use rocket::response::{Redirect, Flash};
use rocket_i18n::I18n;
use std::{
    collections::{HashMap, HashSet},
    borrow::Cow, time::Duration,
};
use validator::{Validate, ValidationError, ValidationErrors};

use plume_common::activity_pub::{broadcast, ActivityStream, ApRequest, inbox::Inbox};
use plume_common::utils;
use plume_models::{
    Context,
    blogs::*,
    db_conn::DbConn,
    Error,
    comments::{Comment, CommentTree},
    instance::Instance,
    medias::Media,
    mentions::Mention,
    post_authors::*,
    posts::*,
    safe_string::SafeString,
    tags::*,
    users::User
};
use routes::{errors::ErrorPage, comments::NewCommentForm, ContentLen};
use template_utils::Ructe;
use Worker;
use Searcher;

#[get("/~/<blog>/<slug>?<responding_to>", rank = 4)]
pub fn details(blog: String, slug: String, conn: DbConn, user: Option<User>, responding_to: Option<i32>, intl: I18n) -> Result<Ructe, ErrorPage> {
    let blog = Blog::find_by_fqn(&*conn, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, blog.id)?;
    if post.published || post.get_authors(&*conn)?.into_iter().any(|a| a.id == user.clone().map(|u| u.id).unwrap_or(0)) {
        let comments = CommentTree::from_post(&*conn, &post, user.as_ref())?;

        let previous = responding_to.and_then(|r| Comment::get(&*conn, r).ok());

        Ok(render!(posts::details(
            &(&*conn, &intl.catalog, user.clone()),
            post.clone(),
            blog,
            &NewCommentForm {
                warning: previous.clone().map(|p| p.spoiler_text).unwrap_or_default(),
                content: previous.clone().and_then(|p| Some(format!(
                    "@{} {}",
                    p.get_author(&*conn).ok()?.get_fqn(&*conn),
                    Mention::list_for_comment(&*conn, p.id).ok()?
                        .into_iter()
                        .filter_map(|m| {
                            let user = user.clone();
                            if let Ok(mentioned) = m.get_mentioned(&*conn) {
                                if user.is_none() || mentioned.id != user.expect("posts::details_response: user error while listing mentions").id {
                                    Some(format!("@{}", mentioned.get_fqn(&*conn)))
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
    } else {
        Ok(render!(errors::not_authorized(
            &(&*conn, &intl.catalog, user.clone()),
            i18n!(intl.catalog, "This post isn't published yet.")
        )))
    }
}

#[get("/~/<blog>/<slug>", rank = 3)]
pub fn activity_details(blog: String, slug: String, conn: DbConn, _ap: ApRequest) -> Result<ActivityStream<LicensedArticle>, Option<String>> {
    let blog = Blog::find_by_fqn(&*conn, &blog).map_err(|_| None)?;
    let post = Post::find_by_slug(&*conn, &slug, blog.id).map_err(|_| None)?;
    if post.published {
        Ok(ActivityStream::new(post.to_activity(&*conn).map_err(|_| String::from("Post serialization error"))?))
    } else {
        Err(Some(String::from("Not published yet.")))
    }
}

#[get("/~/<blog>/new", rank = 2)]
pub fn new_auth(blog: String, i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(i18n.catalog, "You need to be logged in order to write a new post"),
        uri!(new: blog = blog)
    )
}

#[get("/~/<blog>/new", rank = 1)]
pub fn new(blog: String, user: User, cl: ContentLen, conn: DbConn, intl: I18n) -> Result<Ructe, ErrorPage> {
    let b = Blog::find_by_fqn(&*conn, &blog)?;

    if !user.is_author_in(&*conn, &b)? {
        // TODO actually return 403 error code
        Ok(render!(errors::not_authorized(
            &(&*conn, &intl.catalog, Some(user)),
            i18n!(intl.catalog, "You are not author in this blog.")
        )))
    } else {
        let medias = Media::for_user(&*conn, user.id)?;
        Ok(render!(posts::new(
            &(&*conn, &intl.catalog, Some(user)),
            i18n!(intl.catalog, "New post"),
            b,
            false,
            &NewPostForm {
                license: Instance::get_local(&*conn)?.default_license,
                ..NewPostForm::default()
            },
            true,
            None,
            ValidationErrors::default(),
            medias,
            cl.0
        )))
    }
}

#[get("/~/<blog>/<slug>/edit")]
pub fn edit(blog: String, slug: String, user: User, cl: ContentLen, conn: DbConn, intl: I18n) -> Result<Ructe, ErrorPage> {
    let b = Blog::find_by_fqn(&*conn, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, b.id)?;

    if !user.is_author_in(&*conn, &b)? {
        Ok(render!(errors::not_authorized(
            &(&*conn, &intl.catalog, Some(user)),
            i18n!(intl.catalog, "You are not author in this blog.")
        )))
    } else {
        let source = if !post.source.is_empty() {
            post.source.clone()
        } else {
            post.content.get().clone() // fallback to HTML if the markdown was not stored
        };

        let medias = Media::for_user(&*conn, user.id)?;
        let title = post.title.clone();
        Ok(render!(posts::new(
            &(&*conn, &intl.catalog, Some(user)),
            i18n!(intl.catalog, "Edit {0}"; &title),
            b,
            true,
            &NewPostForm {
                title: post.title.clone(),
                subtitle: post.subtitle.clone(),
                content: source,
                tags: Tag::for_post(&*conn, post.id)?
                    .into_iter()
                    .filter_map(|t| if !t.is_hashtag {Some(t.tag)} else {None})
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
}

#[post("/~/<blog>/<slug>/edit", data = "<form>")]
pub fn update(blog: String, slug: String, user: User, cl: ContentLen, form: LenientForm<NewPostForm>, worker: Worker, conn: DbConn, intl: I18n, searcher: Searcher)
    -> Result<Redirect, Ructe> {
    let b = Blog::find_by_fqn(&*conn, &blog).expect("post::update: blog error");
    let mut post = Post::find_by_slug(&*conn, &slug, b.id).expect("post::update: find by slug error");

    let new_slug = if !post.published {
        form.title.to_string().to_kebab_case()
    } else {
        post.slug.clone()
    };

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };

    if new_slug != slug && Post::find_by_slug(&*conn, &new_slug, b.id).is_ok() {
        errors.add("title", ValidationError {
            code: Cow::from("existing_slug"),
            message: Some(Cow::from("A post with the same title already exists.")),
            params: HashMap::new()
        });
    }

    if errors.is_empty() {
        if !user.is_author_in(&*conn, &b).expect("posts::update: is author in error") {
            // actually it's not "Ok"…
            Ok(Redirect::to(uri!(super::blogs::details: name = blog, page = _)))
        } else {
            let (content, mentions, hashtags) = utils::md_to_html(form.content.to_string().as_ref(), &Instance::get_local(&conn).expect("posts::update: Error getting local instance").public_domain);

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
            post.update(&*conn, &searcher).expect("post::update: update error");;

            if post.published {
                post.update_mentions(&conn, mentions.into_iter().filter_map(|m| Mention::build_activity(&conn, &m).ok()).collect())
                    .expect("post::update: mentions error");;
            }

            let tags = form.tags.split(',').map(|t| t.trim().to_camel_case()).filter(|t| !t.is_empty())
                .collect::<HashSet<_>>().into_iter().filter_map(|t| Tag::build_activity(&conn, t).ok()).collect::<Vec<_>>();
            post.update_tags(&conn, tags).expect("post::update: tags error");

            let hashtags = hashtags.into_iter().map(|h| h.to_camel_case()).collect::<HashSet<_>>()
                .into_iter().filter_map(|t| Tag::build_activity(&conn, t).ok()).collect::<Vec<_>>();
            post.update_hashtags(&conn, hashtags).expect("post::update: hashtags error");

            if post.published {
                if newly_published {
                    let act = post.create_activity(&conn).expect("post::update: act error");
                    let dest = User::one_by_instance(&*conn).expect("post::update: dest error");
                    worker.execute(move || broadcast(&user, act, dest));
                } else {
                    let act = post.update_activity(&*conn).expect("post::update: act error");
                    let dest = User::one_by_instance(&*conn).expect("posts::update: dest error");
                    worker.execute(move || broadcast(&user, act, dest));
                }
            }

            Ok(Redirect::to(uri!(details: blog = blog, slug = new_slug, responding_to = _)))
        }
    } else {
        let medias = Media::for_user(&*conn, user.id).expect("posts:update: medias error");
        Err(render!(posts::new(
            &(&*conn, &intl.catalog, Some(user)),
            i18n!(intl.catalog, "Edit {0}"; &form.title),
            b,
            true,
            &*form,
            form.draft.clone(),
            Some(post),
            errors.clone(),
            medias.clone(),
            cl.0
        )))
    }
}

#[derive(Default, FromForm, Validate, Serialize)]
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
pub fn create(blog_name: String, form: LenientForm<NewPostForm>, user: User, cl: ContentLen, conn: DbConn, worker: Worker, intl: I18n, searcher: Searcher) -> Result<Redirect, Result<Ructe, ErrorPage>> {
    let blog = Blog::find_by_fqn(&*conn, &blog_name).expect("post::create: blog error");;
    let slug = form.title.to_string().to_kebab_case();

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };
    if Post::find_by_slug(&*conn, &slug, blog.id).is_ok() {
        errors.add("title", ValidationError {
            code: Cow::from("existing_slug"),
            message: Some(Cow::from("A post with the same title already exists.")),
            params: HashMap::new()
        });
    }

    if errors.is_empty() {
        if !user.is_author_in(&*conn, &blog).expect("post::create: is author in error") {
            // actually it's not "Ok"…
            Ok(Redirect::to(uri!(super::blogs::details: name = blog_name, page = _)))
        } else {
            let (content, mentions, hashtags) = utils::md_to_html(
                form.content.to_string().as_ref(),
                &Instance::get_local(&conn).expect("post::create: local instance error").public_domain
            );

            let post = Post::insert(&*conn, NewPost {
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
                &searcher,
            ).expect("post::create: post save error");

            PostAuthor::insert(&*conn, NewPostAuthor {
                post_id: post.id,
                author_id: user.id
            }).expect("post::create: author save error");

            let tags = form.tags.split(',')
                .map(|t| t.trim().to_camel_case())
                .filter(|t| !t.is_empty())
                .collect::<HashSet<_>>();
            for tag in tags {
                Tag::insert(&*conn, NewTag {
                    tag,
                    is_hashtag: false,
                    post_id: post.id
                }).expect("post::create: tags save error");
            }
            for hashtag in hashtags {
                Tag::insert(&*conn, NewTag {
                    tag: hashtag.to_camel_case(),
                    is_hashtag: true,
                    post_id: post.id
                }).expect("post::create: hashtags save error");
            }

            if post.published {
                for m in mentions {
                    Mention::from_activity(
                        &*conn,
                        &Mention::build_activity(&*conn, &m).expect("post::create: mention build error"),
                        post.id,
                        true,
                        true
                    ).expect("post::create: mention save error");
                }

                let act = post.create_activity(&*conn).expect("posts::create: activity error");
                let dest = User::one_by_instance(&*conn).expect("posts::create: dest error");
                worker.execute(move || broadcast(&user, act, dest));
            }

            Ok(Redirect::to(uri!(details: blog = blog_name, slug = slug, responding_to = _)))
        }
    } else {
        let medias = Media::for_user(&*conn, user.id).expect("posts::create: medias error");
        Err(Ok(render!(posts::new(
            &(&*conn, &intl.catalog, Some(user)),
            i18n!(intl.catalog, "New post"),
            blog,
            false,
            &*form,
            form.draft,
            None,
            errors.clone(),
            medias,
            cl.0
        ))))
    }
}

#[post("/~/<blog_name>/<slug>/delete")]
pub fn delete(blog_name: String, slug: String, conn: DbConn, user: User, worker: Worker, searcher: Searcher) -> Result<Redirect, ErrorPage> {
    let post = Blog::find_by_fqn(&*conn, &blog_name)
        .and_then(|blog| Post::find_by_slug(&*conn, &slug, blog.id));

    if let Ok(post) = post {
        if !post.get_authors(&*conn)?.into_iter().any(|a| a.id == user.id) {
            Ok(Redirect::to(uri!(details: blog = blog_name.clone(), slug = slug.clone(), responding_to = _)))
        } else {
            let dest = User::one_by_instance(&*conn)?;
            let delete_activity = post.build_delete(&*conn)?;
            Inbox::handle(&Context::build(&*conn, &searcher), serde_json::to_value(&delete_activity).map_err(Error::from)?)
                .with::<User, Delete, Post, _>()
                .done()?;

            let user_c = user.clone();
            worker.execute(move || broadcast(&user_c, delete_activity, dest));
            worker.execute_after(Duration::from_secs(10*60), move || {user.rotate_keypair(&conn).expect("Failed to rotate keypair");});

            Ok(Redirect::to(uri!(super::blogs::details: name = blog_name, page = _)))
        }
    } else {
        Ok(Redirect::to(uri!(super::blogs::details: name = blog_name, page = _)))
    }
}
