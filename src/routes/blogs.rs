use activitypub::collection::OrderedCollection;
use atom_syndication::{Entry, FeedBuilder};
use diesel::SaveChangesDsl;
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
use plume_models::{
    blog_authors::*, blogs::*, instance::Instance, medias::*, posts::Post, safe_string::SafeString,
    users::User, Connection, PlumeRocket,
};
use routes::{errors::ErrorPage, Page};
use template_utils::{IntoContext, Ructe};

#[get("/~/<name>?<page>", rank = 2)]
pub fn details(name: String, page: Option<Page>, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let conn = &*rockets.conn;
    let blog = Blog::find_by_fqn(&rockets, &name)?;
    let posts = Post::blog_page(conn, &blog, page.limits())?;
    let articles_count = Post::count_for_blog(conn, &blog)?;
    let authors = &blog.list_authors(conn)?;

    Ok(render!(blogs::details(
        &rockets.to_context(),
        blog,
        authors,
        page.0,
        Page::total(articles_count as i32),
        posts
    )))
}

#[get("/~/<name>", rank = 1)]
pub fn activity_details(
    name: String,
    rockets: PlumeRocket,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomGroup>> {
    let blog = Blog::find_by_fqn(&rockets, &name).ok()?;
    Some(ActivityStream::new(blog.to_activity(&*rockets.conn).ok()?))
}

#[get("/blogs/new")]
pub fn new(rockets: PlumeRocket, _user: User) -> Ructe {
    render!(blogs::new(
        &rockets.to_context(),
        &NewBlogForm::default(),
        ValidationErrors::default()
    ))
}

#[get("/blogs/new", rank = 2)]
pub fn new_auth(i18n: I18n) -> Flash<Redirect> {
    utils::requires_login(
        &i18n!(
            i18n.catalog,
            "To create a new blog, you need to be logged in"
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
pub fn create(
    form: LenientForm<NewBlogForm>,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, Ructe> {
    let slug = utils::make_actor_id(&form.title);
    let conn = &*rockets.conn;
    let intl = &rockets.intl.catalog;
    let user = rockets.user.clone().unwrap();

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
    };
    if Blog::find_by_fqn(&rockets, &slug).is_ok() {
        errors.add(
            "title",
            ValidationError {
                code: Cow::from("existing_slug"),
                message: Some(Cow::from(i18n!(
                    intl,
                    "A blog with the same name already exists."
                ))),
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

        Ok(Flash::success(
            Redirect::to(uri!(details: name = slug.clone(), page = _)),
            &i18n!(intl, "Your blog was successfully created!"),
        ))
    } else {
        Err(render!(blogs::new(&rockets.to_context(), &*form, errors)))
    }
}

#[post("/~/<name>/delete")]
pub fn delete(name: String, rockets: PlumeRocket) -> Result<Flash<Redirect>, Ructe> {
    let conn = &*rockets.conn;
    let blog = Blog::find_by_fqn(&rockets, &name).expect("blog::delete: blog not found");

    if rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&*conn, &blog).ok())
        .unwrap_or(false)
    {
        blog.delete(&conn, &rockets.searcher)
            .expect("blog::expect: deletion error");
        Ok(Flash::success(
            Redirect::to(uri!(super::instance::index)),
            i18n!(rockets.intl.catalog, "Your blog was deleted."),
        ))
    } else {
        // TODO actually return 403 error code
        Err(render!(errors::not_authorized(
            &rockets.to_context(),
            i18n!(
                rockets.intl.catalog,
                "You are not allowed to delete this blog."
            )
        )))
    }
}

#[derive(FromForm, Validate)]
pub struct EditForm {
    #[validate(custom(function = "valid_slug", message = "Invalid name"))]
    pub title: String,
    pub summary: String,
    pub icon: Option<i32>,
    pub banner: Option<i32>,
}

#[get("/~/<name>/edit")]
pub fn edit(name: String, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let conn = &*rockets.conn;
    let blog = Blog::find_by_fqn(&rockets, &name)?;
    if rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(conn, &blog).ok())
        .unwrap_or(false)
    {
        let user = rockets
            .user
            .clone()
            .expect("blogs::edit: User was None while it shouldn't");
        let medias = Media::for_user(conn, user.id).expect("Couldn't list media");
        Ok(render!(blogs::edit(
            &rockets.to_context(),
            &blog,
            medias,
            &EditForm {
                title: blog.title.clone(),
                summary: blog.summary.clone(),
                icon: blog.icon_id,
                banner: blog.banner_id,
            },
            ValidationErrors::default()
        )))
    } else {
        // TODO actually return 403 error code
        Ok(render!(errors::not_authorized(
            &rockets.to_context(),
            i18n!(
                rockets.intl.catalog,
                "You are not allowed to edit this blog."
            )
        )))
    }
}

/// Returns true if the media is owned by `user` and is a picture
fn check_media(conn: &Connection, id: i32, user: &User) -> bool {
    if let Ok(media) = Media::get(conn, id) {
        media.owner_id == user.id && media.category() == MediaCategory::Image
    } else {
        false
    }
}

#[put("/~/<name>/edit", data = "<form>")]
pub fn update(
    name: String,
    form: LenientForm<EditForm>,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, Ructe> {
    let conn = &*rockets.conn;
    let intl = &rockets.intl.catalog;
    let mut blog = Blog::find_by_fqn(&rockets, &name).expect("blog::update: blog not found");
    if rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&*conn, &blog).ok())
        .unwrap_or(false)
    {
        let user = rockets
            .user
            .clone()
            .expect("blogs::edit: User was None while it shouldn't");
        form.validate()
            .and_then(|_| {
                if let Some(icon) = form.icon {
                    if !check_media(&*conn, icon, &user) {
                        let mut errors = ValidationErrors::new();
                        errors.add(
                            "",
                            ValidationError {
                                code: Cow::from("icon"),
                                message: Some(Cow::from(i18n!(
                                    intl,
                                    "You can't use this media as a blog icon."
                                ))),
                                params: HashMap::new(),
                            },
                        );
                        return Err(errors);
                    }
                }

                if let Some(banner) = form.banner {
                    if !check_media(&*conn, banner, &user) {
                        let mut errors = ValidationErrors::new();
                        errors.add(
                            "",
                            ValidationError {
                                code: Cow::from("banner"),
                                message: Some(Cow::from(i18n!(
                                    intl,
                                    "You can't use this media as a blog banner."
                                ))),
                                params: HashMap::new(),
                            },
                        );
                        return Err(errors);
                    }
                }

                blog.title = form.title.clone();
                blog.summary = form.summary.clone();
                blog.summary_html = SafeString::new(
                    &utils::md_to_html(
                        &form.summary,
                        None,
                        true,
                        Some(Media::get_media_processor(
                            &conn,
                            blog.list_authors(&conn)
                                .expect("Couldn't get list of authors")
                                .iter()
                                .collect(),
                        )),
                    )
                    .0,
                );
                blog.icon_id = form.icon;
                blog.banner_id = form.banner;
                blog.save_changes::<Blog>(&*conn)
                    .expect("Couldn't save blog changes");
                Ok(Flash::success(
                    Redirect::to(uri!(details: name = name, page = _)),
                    i18n!(intl, "Your blog information have been updated."),
                ))
            })
            .map_err(|err| {
                let medias = Media::for_user(&*conn, user.id).expect("Couldn't list media");
                render!(blogs::edit(
                    &rockets.to_context(),
                    &blog,
                    medias,
                    &*form,
                    err
                ))
            })
    } else {
        // TODO actually return 403 error code
        Err(render!(errors::not_authorized(
            &rockets.to_context(),
            i18n!(
                rockets.intl.catalog,
                "You are not allowed to edit this blog."
            )
        )))
    }
}

#[get("/~/<name>/outbox")]
pub fn outbox(name: String, rockets: PlumeRocket) -> Option<ActivityStream<OrderedCollection>> {
    let blog = Blog::find_by_fqn(&rockets, &name).ok()?;
    Some(blog.outbox(&*rockets.conn).ok()?)
}

#[get("/~/<name>/atom.xml")]
pub fn atom_feed(name: String, rockets: PlumeRocket) -> Option<Content<String>> {
    let blog = Blog::find_by_fqn(&rockets, &name).ok()?;
    let conn = &*rockets.conn;
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
