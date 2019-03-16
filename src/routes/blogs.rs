use activitypub::collection::OrderedCollection;
use atom_syndication::{Entry, FeedBuilder};
use rocket::{
    http::ContentType,
    request::LenientForm,
    response::{Redirect, Flash, content::Content}
};
use rocket_i18n::I18n;
use std::{collections::HashMap, borrow::Cow};
use validator::{Validate, ValidationError, ValidationErrors};

use plume_common::activity_pub::{ActivityStream, ApRequest};
use plume_common::utils;
use plume_models::{
    Context,
    blog_authors::*,
    blogs::*,
    db_conn::DbConn,
    instance::Instance,
    posts::Post,
    users::User
};
use routes::{Page, errors::ErrorPage};
use template_utils::Ructe;
use Searcher;

#[get("/~/<name>?<page>", rank = 2)]
pub fn details(intl: I18n, name: String, conn: DbConn, user: Option<User>, page: Option<Page>, searcher: Searcher) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let blog = Blog::find_by_fqn(&Context::build(&*conn, &*searcher), &name)?;
    let posts = Post::blog_page(&*conn, &blog, page.limits())?;
    let articles_count = Post::count_for_blog(&*conn, &blog)?;
    let authors = &blog.list_authors(&*conn)?;

    Ok(render!(blogs::details(
        &(&*conn, &intl.catalog, user.clone()),
        blog.clone(),
        authors,
        articles_count,
        page.0,
        Page::total(articles_count as i32),
        user.and_then(|x| x.is_author_in(&*conn, &blog).ok()).unwrap_or(false),
        posts
    )))
}

#[get("/~/<name>", rank = 1)]
pub fn activity_details(name: String, conn: DbConn, _ap: ApRequest, searcher: Searcher) -> Option<ActivityStream<CustomGroup>> {
    let blog = Blog::find_by_fqn(&Context::build(&*conn, &*searcher), &name).ok()?;
    Some(ActivityStream::new(blog.to_activity(&*conn).ok()?))
}

#[get("/blogs/new")]
pub fn new(user: User, conn: DbConn, intl: I18n) -> Ructe {
    render!(blogs::new(
        &(&*conn, &intl.catalog, Some(user)),
        &NewBlogForm::default(),
        ValidationErrors::default()
    ))
}

#[get("/blogs/new", rank = 2)]
pub fn new_auth(i18n: I18n) -> Flash<Redirect>{
    utils::requires_login(
        &i18n!(i18n.catalog, "You need to be logged in order to create a new blog"),
        uri!(new)
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
pub fn create(conn: DbConn, form: LenientForm<NewBlogForm>, user: User, intl: I18n, searcher: Searcher) -> Result<Redirect, Ructe> {
    let slug = utils::make_actor_id(&form.title);

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };
    if Blog::find_by_fqn(&Context::build(&*conn, &*searcher), &slug).is_ok() {
        errors.add("title", ValidationError {
            code: Cow::from("existing_slug"),
            message: Some(Cow::from("A blog with the same name already exists.")),
            params: HashMap::new()
        });
    }

    if errors.is_empty() {
        let blog = Blog::insert(&*conn, NewBlog::new_local(
            slug.clone(),
            form.title.to_string(),
            String::from(""),
            Instance::get_local(&*conn).expect("blog::create: instance error").id
        ).expect("blog::create: new local error")).expect("blog::create:  error");

        BlogAuthor::insert(&*conn, NewBlogAuthor {
            blog_id: blog.id,
            author_id: user.id,
            is_owner: true
        }).expect("blog::create: author error");

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
pub fn delete(conn: DbConn, name: String, user: Option<User>, intl: I18n, searcher: Searcher) -> Result<Redirect, Ructe>{
    let blog = Blog::find_by_fqn(&Context::build(&*conn, &*searcher), &name).expect("blog::delete: blog not found");
    if user.clone().and_then(|u| u.is_author_in(&*conn, &blog).ok()).unwrap_or(false) {
        blog.delete(&conn, &searcher).expect("blog::expect: deletion error");
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
pub fn outbox(name: String, conn: DbConn, searcher: Searcher) -> Option<ActivityStream<OrderedCollection>> {
    let blog = Blog::find_by_fqn(&Context::build(&*conn, &*searcher), &name).ok()?;
    Some(blog.outbox(&*conn).ok()?)
}

#[get("/~/<name>/atom.xml")]
pub fn atom_feed(name: String, conn: DbConn, searcher: Searcher) -> Option<Content<String>> {
    let blog = Blog::find_by_fqn(&Context::build(&*conn, &*searcher), &name).ok()?;
    let feed = FeedBuilder::default()
        .title(blog.title.clone())
        .id(Instance::get_local(&*conn).ok()?
            .compute_box("~", &name, "atom.xml"))
        .entries(Post::get_recents_for_blog(&*conn, &blog, 15).ok()?
            .into_iter()
            .map(|p| super::post_to_atom(p, &*conn))
            .collect::<Vec<Entry>>())
        .build().ok()?;
    Some(Content(ContentType::new("application", "atom+xml"), feed.to_string()))
}
