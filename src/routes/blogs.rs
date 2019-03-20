use activitypub::collection::OrderedCollection;
use atom_syndication::{Entry, FeedBuilder};
use rocket::{
    http::ContentType,
    request::LenientForm,
    response::{content::Content, Flash, Redirect},
};
use rocket_i18n::I18n;
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use plume_common::activity_pub::{ActivityStream, ApRequest};
use plume_common::utils;
use plume_models::{blog_authors::*, blogs::*, db_conn::DbConn, instance::Instance, posts::Post};
use routes::{errors::ErrorPage, Page, PlumeRocket};
use template_utils::Ructe;

#[get("/~/<name>?<page>", rank = 2)]
pub fn details(name: String, page: Option<Page>, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let conn = rockets.conn;
    let blog = Blog::find_by_fqn(&*conn, &name)?;
    let posts = Post::blog_page(&*conn, &blog, page.limits())?;
    let articles_count = Post::count_for_blog(&*conn, &blog)?;
    let authors = &blog.list_authors(&*conn)?;
    let user = rockets.user;
    let intl = rockets.intl;

    Ok(render!(blogs::details(
        &(&*conn, &intl.catalog, user.clone()),
        blog.clone(),
        authors,
        articles_count,
        page.0,
        Page::total(articles_count as i32),
        user.and_then(|x| x.is_author_in(&*conn, &blog).ok())
            .unwrap_or(false),
        posts
    )))
}

#[get("/~/<name>", rank = 1)]
pub fn activity_details(
    name: String,
    conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomGroup>> {
    let blog = Blog::find_by_fqn(&*conn, &name).ok()?;
    Some(ActivityStream::new(blog.to_activity(&*conn).ok()?))
}

#[get("/blogs/new")]
pub fn new(rockets: PlumeRocket) -> Ructe {
    let user = rockets.user.unwrap();
    let intl = rockets.intl;
    let conn = rockets.conn;

    render!(blogs::new(
        &(&*conn, &intl.catalog, Some(user)),
        &NewBlogForm::default(),
        ValidationErrors::default()
    ))
}

#[get("/blogs/new", rank = 2)]
pub fn new_auth(i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(
            i18n.catalog,
            "You need to be logged in order to create a new blog"
        ),
        uri!(new),
    )
}

#[derive(Default, FromForm, Validate)]
pub struct NewBlogForm {
    #[validate(custom(function = "valid_slug", message = "Invalid name"))]
    pub title: String,
}

fn valid_slug(title: &str) -> Result<(), ValidationError> {
    let slug = utils::make_actor_id(title);
    if slug.is_empty() {
        Err(ValidationError::new("empty_slug"))
    } else {
        Ok(())
    }
}

#[post("/blogs/new", data = "<form>")]
pub fn create(form: LenientForm<NewBlogForm>, rockets: PlumeRocket) -> Result<Redirect, Ructe> {
    let slug = utils::make_actor_id(&form.title);
    let conn = rockets.conn;
    let intl = rockets.intl;
    let user = rockets.user.unwrap();

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
    };
    if Blog::find_by_fqn(&*conn, &slug).is_ok() {
        errors.add(
            "title",
            ValidationError {
                code: Cow::from("existing_slug"),
                message: Some(Cow::from("A blog with the same name already exists.")),
                params: HashMap::new(),
            },
        );
    }

    if errors.is_empty() {
        let blog = Blog::insert(
            &*conn,
            NewBlog::new_local(
                slug.clone(),
                form.title.to_string(),
                String::from(""),
                Instance::get_local(&*conn)
                    .expect("blog::create: instance error")
                    .id,
            )
            .expect("blog::create: new local error"),
        )
        .expect("blog::create:  error");

        BlogAuthor::insert(
            &*conn,
            NewBlogAuthor {
                blog_id: blog.id,
                author_id: user.id,
                is_owner: true,
            },
        )
        .expect("blog::create: author error");

        Ok(Redirect::to(uri!(details: name = slug.clone(), page = _)))
    } else {
        Err(render!(blogs::new(
            &(&*conn, &intl.catalog, Some(user)),
            &*form,
            errors
        )))
    }
}

#[post("/~/<name>/delete")]
pub fn delete(name: String, rockets: PlumeRocket) -> Result<Redirect, Ructe> {
    let conn = rockets.conn;
    let blog = Blog::find_by_fqn(&*conn, &name).expect("blog::delete: blog not found");
    let user = rockets.user;
    let intl = rockets.intl;
    let searcher = rockets.searcher;

    if user
        .clone()
        .and_then(|u| u.is_author_in(&*conn, &blog).ok())
        .unwrap_or(false)
    {
        blog.delete(&conn, &searcher)
            .expect("blog::expect: deletion error");
        Ok(Redirect::to(uri!(super::instance::index)))
    } else {
        // TODO actually return 403 error code
        Err(render!(errors::not_authorized(
            &(&*conn, &intl.catalog, user),
            i18n!(intl.catalog, "You are not allowed to delete this blog.")
        )))
    }
}

#[get("/~/<name>/outbox")]
pub fn outbox(name: String, conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let blog = Blog::find_by_fqn(&*conn, &name).ok()?;
    Some(blog.outbox(&*conn).ok()?)
}

#[get("/~/<name>/atom.xml")]
pub fn atom_feed(name: String, conn: DbConn) -> Option<Content<String>> {
    let blog = Blog::find_by_fqn(&*conn, &name).ok()?;
    let feed = FeedBuilder::default()
        .title(blog.title.clone())
        .id(Instance::get_local(&*conn)
            .ok()?
            .compute_box("~", &name, "atom.xml"))
        .entries(
            Post::get_recents_for_blog(&*conn, &blog, 15)
                .ok()?
                .into_iter()
                .map(|p| super::post_to_atom(p, &*conn))
                .collect::<Vec<Entry>>(),
        )
        .build()
        .ok()?;
    Some(Content(
        ContentType::new("application", "atom+xml"),
        feed.to_string(),
    ))
}
