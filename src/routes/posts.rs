use activitypub::object::Article;
use chrono::Utc;
use heck::{CamelCase, KebabCase};
use rocket::{State, request::LenientForm};
use rocket::response::{Redirect, Flash};
use rocket_contrib::Template;
use serde_json;
use std::{collections::{HashMap, HashSet}, borrow::Cow};
use validator::{Validate, ValidationError, ValidationErrors};
use workerpool::{Pool, thunk::*};

use plume_common::activity_pub::{broadcast, ActivityStream, ApRequest, inbox::Deletable};
use plume_common::utils;
use plume_models::{
    blogs::*,
    db_conn::DbConn,
    comments::Comment,
    instance::Instance,
    medias::Media,
    mentions::Mention,
    post_authors::*,
    posts::*,
    safe_string::SafeString,
    tags::*,
    users::User
};

#[derive(FromForm)]
struct CommentQuery {
    responding_to: Option<i32>
}

// See: https://github.com/SergioBenitez/Rocket/pull/454
#[get("/~/<blog>/<slug>", rank = 4)]
fn details(blog: String, slug: String, conn: DbConn, user: Option<User>) -> Template {
    details_response(blog, slug, conn, user, None)
}

#[get("/~/<blog>/<slug>?<query>")]
fn details_response(blog: String, slug: String, conn: DbConn, user: Option<User>, query: Option<CommentQuery>) -> Template {
    may_fail!(user.map(|u| u.to_json(&*conn)), Blog::find_by_fqn(&*conn, &blog), "Couldn't find this blog", |blog| {
        may_fail!(user.map(|u| u.to_json(&*conn)), Post::find_by_slug(&*conn, &slug, blog.id), "Couldn't find this post", |post| {
            if post.published || post.get_authors(&*conn).into_iter().any(|a| a.id == user.clone().map(|u| u.id).unwrap_or(0)) {
                let comments = Comment::list_by_post(&*conn, post.id);
                let comms = comments.clone();

                let previous = query.and_then(|q| q.responding_to.map(|r| Comment::get(&*conn, r)
                    .expect("posts::details_reponse: Error retrieving previous comment").to_json(&*conn, &[])));
                Template::render("posts/details", json!({
                    "author": post.get_authors(&*conn)[0].to_json(&*conn),
                    "article": post.to_json(&*conn),
                    "blog": blog.to_json(&*conn),
                    "comments": &comments.into_iter().filter_map(|c| if c.in_response_to_id.is_none() {
                        Some(c.to_json(&*conn, &comms))
                    } else {
                        None
                    }).collect::<Vec<serde_json::Value>>(),
                    "n_likes": post.get_likes(&*conn).len(),
                    "has_liked": user.clone().map(|u| u.has_liked(&*conn, &post)).unwrap_or(false),
                    "n_reshares": post.get_reshares(&*conn).len(),
                    "has_reshared": user.clone().map(|u| u.has_reshared(&*conn, &post)).unwrap_or(false),
                    "account": &user.clone().map(|u| u.to_json(&*conn)),
                    "date": &post.creation_date.timestamp(),
                    "previous": previous,
                    "default": {
                        "warning": previous.map(|p| p["spoiler_text"].clone())
                    },
                    "user_fqn": user.clone().map(|u| u.get_fqn(&*conn)).unwrap_or_default(),
                    "is_author": user.clone().map(|u| post.get_authors(&*conn).into_iter().any(|a| u.id == a.id)).unwrap_or(false),
                    "is_following": user.map(|u| u.is_following(&*conn, post.get_authors(&*conn)[0].id)).unwrap_or(false),
                    "comment_form": null,
                    "comment_errors": null,
                }))
            } else {
                Template::render("errors/403", json!({
                    "error_message": "This post isn't published yet."
                }))
            }
        })
    })
}

#[get("/~/<blog>/<slug>", rank = 3)]
fn activity_details(blog: String, slug: String, conn: DbConn, _ap: ApRequest) -> Result<ActivityStream<Article>, Option<String>> {
    let blog = Blog::find_by_fqn(&*conn, &blog).ok_or(None)?;
    let post = Post::find_by_slug(&*conn, &slug, blog.id).ok_or(None)?;
    if post.published {
        Ok(ActivityStream::new(post.to_activity(&*conn)))
    } else {
        Err(Some(String::from("Not published yet.")))
    }
}

#[get("/~/<blog>/new", rank = 2)]
fn new_auth(blog: String) -> Flash<Redirect> {
    utils::requires_login(
        "You need to be logged in order to write a new post",
        uri!(new: blog = blog)
    )
}

#[get("/~/<blog>/new", rank = 1)]
fn new(blog: String, user: User, conn: DbConn) -> Option<Template> {
    let b = Blog::find_by_fqn(&*conn, &blog)?;

    if !user.is_author_in(&*conn, &b) {
        Some(Template::render("errors/403", json!({// TODO actually return 403 error code
            "error_message": "You are not author in this blog."
        })))
    } else {
        let medias = Media::for_user(&*conn, user.id);
        Some(Template::render("posts/new", json!({
            "account": user.to_json(&*conn),
            "instance": Instance::get_local(&*conn),
            "editing": false,
            "errors": null,
            "form": null,
            "is_draft": true,
            "medias": medias.into_iter().map(|m| m.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
        })))
    }
}

#[get("/~/<blog>/<slug>/edit")]
fn edit(blog: String, slug: String, user: User, conn: DbConn) -> Option<Template> {
    let b = Blog::find_by_fqn(&*conn, &blog)?;
    let post = Post::find_by_slug(&*conn, &slug, b.id)?;

    if !user.is_author_in(&*conn, &b) {
        Some(Template::render("errors/403", json!({// TODO actually return 403 error code
            "error_message": "You are not author in this blog."
        })))
    } else {
        let source = if !post.source.is_empty() {
            post.source
        } else {
            post.content.get().clone() // fallback to HTML if the markdown was not stored
        };

        let medias = Media::for_user(&*conn, user.id);
        Some(Template::render("posts/new", json!({
            "account": user.to_json(&*conn),
            "instance": Instance::get_local(&*conn),
            "editing": true,
            "errors": null,
            "form": NewPostForm {
                title: post.title.clone(),
                subtitle: post.subtitle.clone(),
                content: source,
                tags: Tag::for_post(&*conn, post.id)
                    .into_iter()
                    .filter_map(|t| if !t.is_hashtag {Some(t.tag)} else {None})
                    .collect::<Vec<String>>()
                    .join(", "),
                license: post.license.clone(),
                draft: true,
                cover: post.cover_id,
            },
            "is_draft": !post.published,
            "medias": medias.into_iter().map(|m| m.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
        })))
    }
}

#[post("/~/<blog>/<slug>/edit", data = "<data>")]
fn update(blog: String, slug: String, user: User, conn: DbConn, data: LenientForm<NewPostForm>, worker: State<Pool<ThunkWorker<()>>>)
    -> Result<Redirect, Option<Template>> {
    let b = Blog::find_by_fqn(&*conn, &blog).ok_or(None)?;
    let mut post = Post::find_by_slug(&*conn, &slug, b.id).ok_or(None)?;

    let form = data.get();
    let new_slug = if !post.published {
        form.title.to_string().to_kebab_case()
    } else {
        post.slug
    };

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };

    if new_slug != slug && Post::find_by_slug(&*conn, &new_slug, b.id).is_some() {
        errors.add("title", ValidationError {
            code: Cow::from("existing_slug"),
            message: Some(Cow::from("A post with the same title already exists.")),
            params: HashMap::new()
        });
    }

    if errors.is_empty() {
        if !user.is_author_in(&*conn, &b) {
            // actually it's not "Ok"…
            Ok(Redirect::to(uri!(super::blogs::details: name = blog)))
        } else {
            let (content, mentions, hashtags) = utils::md_to_html(form.content.to_string().as_ref());

            let license = if !form.license.is_empty() {
                form.license.to_string()
            } else {
                Instance::get_local(&*conn).map(|i| i.default_license).unwrap_or_else(|| String::from("CC-BY-SA"))
            };

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
            post.license = license;
            post.cover_id = form.cover;
            post.update(&*conn);
            let post = post.update_ap_url(&*conn);

            if post.published {
                post.update_mentions(&conn, mentions.into_iter().map(|m| Mention::build_activity(&conn, &m)).collect());
            }

            let tags = form.tags.split(',').map(|t| t.trim().to_camel_case()).filter(|t| !t.is_empty())
                .collect::<HashSet<_>>().into_iter().map(|t| Tag::build_activity(&conn, t)).collect::<Vec<_>>();
            post.update_tags(&conn, tags);

            let hashtags = hashtags.into_iter().map(|h| h.to_camel_case()).collect::<HashSet<_>>()
                .into_iter().map(|t| Tag::build_activity(&conn, t)).collect::<Vec<_>>();
            post.update_tags(&conn, hashtags);

            if post.published {
                if newly_published {
                    let act = post.create_activity(&conn);
                    let dest = User::one_by_instance(&*conn);
                    worker.execute(Thunk::of(move || broadcast(&user, act, dest)));
                } else {
                    let act = post.update_activity(&*conn);
                    let dest = User::one_by_instance(&*conn);
                    worker.execute(Thunk::of(move || broadcast(&user, act, dest)));
                }
            }

            Ok(Redirect::to(uri!(details: blog = blog, slug = new_slug)))
        }
    } else {
        let medias = Media::for_user(&*conn, user.id);
        Err(Some(Template::render("posts/new", json!({
            "account": user.to_json(&*conn),
            "instance": Instance::get_local(&*conn),
            "editing": true,
            "errors": errors.field_errors(),
            "form": form,
            "is_draft": form.draft,
            "medias": medias.into_iter().map(|m| m.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
        }))))
    }
}

#[derive(FromForm, Validate, Serialize)]
struct NewPostForm {
    #[validate(custom(function = "valid_slug", message = "Invalid title"))]
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub tags: String,
    pub license: String,
    pub draft: bool,
    pub cover: Option<i32>,
}

fn valid_slug(title: &str) -> Result<(), ValidationError> {
    let slug = title.to_string().to_kebab_case();
    if slug.is_empty() {
        Err(ValidationError::new("empty_slug"))
    } else if slug == "new" {
        Err(ValidationError::new("invalid_slug"))
    } else {
        Ok(())
    }
}

#[post("/~/<blog_name>/new", data = "<data>")]
fn create(blog_name: String, data: LenientForm<NewPostForm>, user: User, conn: DbConn, worker: State<Pool<ThunkWorker<()>>>) -> Result<Redirect, Option<Template>> {
    let blog = Blog::find_by_fqn(&*conn, &blog_name).ok_or(None)?;
    let form = data.get();
    let slug = form.title.to_string().to_kebab_case();

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };
    if Post::find_by_slug(&*conn, &slug, blog.id).is_some() {
        errors.add("title", ValidationError {
            code: Cow::from("existing_slug"),
            message: Some(Cow::from("A post with the same title already exists.")),
            params: HashMap::new()
        });
    }

    if errors.is_empty() {
        if !user.is_author_in(&*conn, &blog) {
            // actually it's not "Ok"…
            Ok(Redirect::to(uri!(super::blogs::details: name = blog_name)))
        } else {
            let (content, mentions, hashtags) = utils::md_to_html(form.content.to_string().as_ref());

            let post = Post::insert(&*conn, NewPost {
                blog_id: blog.id,
                slug: slug.to_string(),
                title: form.title.to_string(),
                content: SafeString::new(&content),
                published: !form.draft,
                license: if !form.license.is_empty() {
                    form.license.to_string()
                } else {
                    Instance::get_local(&*conn).map(|i| i.default_license).unwrap_or_else(||String::from("CC-BY-SA"))
                },
                ap_url: "".to_string(),
                creation_date: None,
                subtitle: form.subtitle.clone(),
                source: form.content.clone(),
                cover_id: form.cover,
            });
            let post = post.update_ap_url(&*conn);
            PostAuthor::insert(&*conn, NewPostAuthor {
                post_id: post.id,
                author_id: user.id
            });

            let tags = form.tags.split(',')
                .map(|t| t.trim().to_camel_case())
                .filter(|t| !t.is_empty())
                .collect::<HashSet<_>>();
            for tag in tags {
                Tag::insert(&*conn, NewTag {
                    tag,
                    is_hashtag: false,
                    post_id: post.id
                });
            }
            for hashtag in hashtags {
                Tag::insert(&*conn, NewTag {
                    tag: hashtag.to_camel_case(),
                    is_hashtag: true,
                    post_id: post.id
                });
            }

            if post.published {
                for m in mentions {
                    Mention::from_activity(&*conn, &Mention::build_activity(&*conn, &m), post.id, true, true);
                }

                let act = post.create_activity(&*conn);
                let dest = User::one_by_instance(&*conn);
                worker.execute(Thunk::of(move || broadcast(&user, act, dest)));
            }

            Ok(Redirect::to(uri!(details: blog = blog_name, slug = slug)))
        }
    } else {
        let medias = Media::for_user(&*conn, user.id);
        Err(Some(Template::render("posts/new", json!({
            "account": user.to_json(&*conn),
            "instance": Instance::get_local(&*conn),
            "editing": false,
            "errors": errors.field_errors(),
            "form": form,
            "is_draft": form.draft,
            "medias": medias.into_iter().map(|m| m.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
        }))))
    }
}

#[post("/~/<blog_name>/<slug>/delete")]
fn delete(blog_name: String, slug: String, conn: DbConn, user: User, worker: State<Pool<ThunkWorker<()>>>) -> Redirect {
    let post = Blog::find_by_fqn(&*conn, &blog_name)
        .and_then(|blog| Post::find_by_slug(&*conn, &slug, blog.id));

    if let Some(post) = post {
        if !post.get_authors(&*conn).into_iter().any(|a| a.id == user.id) {
            Redirect::to(uri!(details: blog = blog_name.clone(), slug = slug.clone()))
        } else {
            let dest = User::one_by_instance(&*conn);
            let delete_activity = post.delete(&*conn);
            worker.execute(Thunk::of(move || broadcast(&user, delete_activity, dest)));

            Redirect::to(uri!(super::blogs::details: name = blog_name))
        }
    } else {
        Redirect::to(uri!(super::blogs::details: name = blog_name))
    }
}
