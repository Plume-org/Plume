use activitypub::collection::OrderedCollection;
use atom_syndication::{Entry, FeedBuilder};
use rocket::{
    http::ContentType,
    request::LenientForm,
    response::{Redirect, Flash, content::Content}
};
use rocket_contrib::templates::Template;
use rocket_i18n::I18n;
use std::{collections::HashMap, borrow::Cow};
use validator::{Validate, ValidationError, ValidationErrors};

use plume_common::activity_pub::{ActivityStream, ApRequest};
use plume_common::utils;
use plume_models::{
    blog_authors::*,
    blogs::*,
    db_conn::DbConn,
    instance::Instance,
    posts::Post,
    users::User
};
use routes::{Page, Ructe};

#[get("/~/<name>?<page>", rank = 2)]
pub fn paginated_details(intl: I18n, name: String, conn: DbConn, user: Option<User>, page: Page) -> Result<Ructe, Ructe> {
    let blog = Blog::find_by_fqn(&*conn, &name)
        .ok_or_else(|| render!(errors::not_found(&(&*conn, &intl.catalog, user.clone()))))?;
    let posts = Post::blog_page(&*conn, &blog, page.limits());
    let articles = Post::get_for_blog(&*conn, &blog); // TODO only count them in DB
    let authors = &blog.list_authors(&*conn);

    Ok(render!(blogs::details(
        &(&*conn, &intl.catalog, user.clone()),
        blog.clone(),
        blog.get_fqn(&*conn),
        authors,
        articles.len(),
        page.0,
        Page::total(articles.len() as i32),
        user.map(|x| x.is_author_in(&*conn, &blog)).unwrap_or(false),
        posts
    )))
}

#[get("/~/<name>", rank = 3)]
pub fn details(intl: I18n, name: String, conn: DbConn, user: Option<User>) -> Result<Ructe, Ructe> {
    paginated_details(intl, name, conn, user, Page::first())
}

#[get("/~/<name>", rank = 1)]
pub fn activity_details(name: String, conn: DbConn, _ap: ApRequest) -> Option<ActivityStream<CustomGroup>> {
    let blog = Blog::find_local(&*conn, &name)?;
    Some(ActivityStream::new(blog.to_activity(&*conn)))
}

#[get("/blogs/new")]
pub fn new(user: User, conn: DbConn, intl: I18n) -> Ructe {
    render!(blogs::new(
        &(&*conn, &intl.catalog, Some(user)),
        NewBlogForm::default(),
        ValidationErrors::default()
    ))
}

#[get("/blogs/new", rank = 2)]
pub fn new_auth() -> Flash<Redirect>{
    utils::requires_login(
        "You need to be logged in order to create a new blog",
        uri!(new)
    )
}

#[derive(Default, FromForm, Validate, Serialize)]
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
pub fn create(conn: DbConn, form: LenientForm<NewBlogForm>, user: User) -> Result<Redirect, Template> {
    let slug = utils::make_actor_id(&form.title);

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };
    if Blog::find_local(&*conn, &slug).is_some() {
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
            Instance::local_id(&*conn)
        ));
        blog.update_boxes(&*conn);

        BlogAuthor::insert(&*conn, NewBlogAuthor {
            blog_id: blog.id,
            author_id: user.id,
            is_owner: true
        });

        Ok(Redirect::to(uri!(details: name = slug.clone())))
    } else {
        println!("{:?}", errors);
        Err(Template::render("blogs/new", json!({
            "account": user.to_json(&*conn),
            "errors": errors.errors(),
            "form": *form,
        })))
    }
}

#[post("/~/<name>/delete")]
pub fn delete(conn: DbConn, name: String, user: Option<User>, intl: I18n) -> Result<Redirect, Option<Ructe>>{
    let blog = Blog::find_local(&*conn, &name).ok_or(None)?;
    if user.clone().map(|u| u.is_author_in(&*conn, &blog)).unwrap_or(false) {
        blog.delete(&conn);
        Ok(Redirect::to(uri!(super::instance::index)))
    } else {
        // TODO actually return 403 error code
        Err(Some(render!(errors::not_authorized(
            &(&*conn, &intl.catalog, user),
            "You are not allowed to delete this blog."
        ))))
    }
}

#[get("/~/<name>/outbox")]
pub fn outbox(name: String, conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let blog = Blog::find_local(&*conn, &name)?;
    Some(blog.outbox(&*conn))
}

#[get("/~/<name>/atom.xml")]
pub fn atom_feed(name: String, conn: DbConn) -> Option<Content<String>> {
    let blog = Blog::find_by_fqn(&*conn, &name)?;
    let feed = FeedBuilder::default()
        .title(blog.title.clone())
        .id(Instance::get_local(&*conn).expect("blogs::atom_feed: local instance not found error")
            .compute_box("~", &name, "atom.xml"))
        .entries(Post::get_recents_for_blog(&*conn, &blog, 15)
            .into_iter()
            .map(|p| super::post_to_atom(p, &*conn))
            .collect::<Vec<Entry>>())
        .build()
        .expect("blogs::atom_feed: feed creation error");
    Some(Content(ContentType::new("application", "atom+xml"), feed.to_string()))
}
