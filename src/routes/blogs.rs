use activitystreams::collection::{OrderedCollection, OrderedCollectionPage};
use rocket::{
    http::ContentType,
    form::Form,
    response::{Flash, Redirect},
};
use rocket_i18n::I18n;
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::routes::{errors::ErrorPage, Page, RespondOrRedirect};
use crate::template_utils::{IntoContext, Ructe, PostCard};
use crate::utils::requires_login;
use plume_common::activity_pub::{ActivityStream, ApRequest, CustomGroup};
use plume_common::utils;
use plume_models::{
    blog_authors::*, blogs::*, db_conn::DbConn, instance::Instance, medias::*, posts::Post,
    safe_string::SafeString, users::User, Connection, PlumeRocket,
};

#[get("/~/<name>?<page>", rank = 2)]
pub async fn details(
    name: String,
    page: Option<crate::routes::Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let blog = Blog::find_by_fqn(&mut conn, &name).await?;
    let blog_icon_url = blog.icon_url(&mut conn);
    let blog_banner_url = blog.banner_url(&mut conn);
    let articles_count = Post::count_for_blog(&mut conn, &blog)?;
    let authors = &blog.list_authors(&mut conn)?;
    let is_author = rockets.user.as_ref().and_then(|u| u.is_author_in(&mut conn, &blog).ok()).unwrap_or(false);

    let blog_pages = Post::blog_page(&mut conn, &blog, page.limits())?;
    let posts = PostCard::from_posts(&mut conn, blog_pages, &rockets.user);

    Ok(render!(blogs::details_html(
        &(&mut conn, &rockets).to_context(),
        blog,
        blog_icon_url,
        blog_banner_url,
        authors,
        is_author,
        page.0,
        Page::total(articles_count as i32),
        posts
    )))
}

#[get("/~/<name>", rank = 1)]
pub async fn activity_details(
    name: String,
    mut conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomGroup>> {
    let blog = Blog::find_by_fqn(&mut conn, &name).await.ok()?;
    Some(ActivityStream::new(blog.to_activity(&mut conn).ok()?))
}

#[get("/blogs/new")]
pub fn new(mut conn: DbConn, rockets: PlumeRocket, _user: User) -> Ructe {
    render!(blogs::new_html(
        &(&mut conn, &rockets).to_context(),
        &NewBlogForm::default(),
        ValidationErrors::default()
    ))
}

#[get("/blogs/new", rank = 2)]
pub fn new_auth(i18n: I18n) -> Flash<Redirect> {
    requires_login(
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
    let slug = Blog::slug(title);
    if slug.is_empty() {
        Err(ValidationError::new("empty_slug"))
    } else {
        Ok(())
    }
}

#[post("/blogs/new", data = "<form>")]
pub async fn create(
    form: Form<NewBlogForm>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> RespondOrRedirect {
    let slug = Blog::slug(&form.title);
    let intl = &rockets.intl.catalog;
    let user = rockets.user.clone().unwrap();

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
    };
    if Blog::find_by_fqn(&mut conn, slug).await.is_ok() {
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

    if !errors.is_empty() {
        return render!(blogs::new_html(&(&mut conn, &rockets).to_context(), &*form, errors)).into();
    }

    let blog = Blog::insert(
        &mut conn,
        NewBlog::new_local(
            slug.into(),
            form.title.to_string(),
            String::from(""),
            Instance::get_local()
                .expect("blog::create: instance error")
                .id,
        )
        .expect("blog::create: new local error"),
    )
    .expect("blog::create:  error");

    BlogAuthor::insert(
        &mut conn,
        NewBlogAuthor {
            blog_id: blog.id,
            author_id: user.id,
            is_owner: true,
        },
    )
    .expect("blog::create: author error");

    Flash::success(
        Redirect::to(uri!(details(name = slug, page = _))),
        &i18n!(intl, "Your blog was successfully created!"),
    )
    .into()
}

#[post("/~/<name>/delete")]
pub async fn delete(name: &str, mut conn: DbConn, rockets: PlumeRocket) -> RespondOrRedirect {
    let blog = Blog::find_by_fqn(&mut conn, name).await.expect("blog::delete: blog not found");

    if rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&mut conn, &blog).ok())
        .unwrap_or(false)
    {
        blog.delete(&mut conn).expect("blog::expect: deletion error");
        Flash::success(
            Redirect::to(uri!(super::instance::index)),
            i18n!(rockets.intl.catalog, "Your blog was deleted."),
        )
        .into()
    } else {
        // TODO actually return 403 error code
        render!(errors::not_authorized_html(
            &(&mut conn, &rockets).to_context(),
            i18n!(
                rockets.intl.catalog,
                "You are not allowed to delete this blog."
            )
        ))
        .into()
    }
}

#[derive(FromForm, Validate)]
pub struct EditForm {
    #[validate(custom(function = "valid_slug", message = "Invalid name"))]
    pub title: String,
    pub summary: String,
    pub icon: Option<i32>,
    pub banner: Option<i32>,
    pub theme: Option<String>,
}

#[get("/~/<name>/edit")]
pub async fn edit(name: &str, mut conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let blog = Blog::find_by_fqn(&mut conn, name).await?;
    if rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&mut conn, &blog).ok())
        .unwrap_or(false)
    {
        let user = rockets
            .user
            .clone()
            .expect("blogs::edit: User was None while it shouldn't");
        let medias = Media::for_user(&mut conn, user.id).expect("Couldn't list media");
        Ok(render!(blogs::edit_html(
            &(&mut conn, &rockets).to_context(),
            &blog,
            medias,
            &EditForm {
                title: blog.title.clone(),
                summary: blog.summary.clone(),
                icon: blog.icon_id,
                banner: blog.banner_id,
                theme: blog.theme.clone(),
            },
            ValidationErrors::default()
        )))
    } else {
        // TODO actually return 403 error code
        Ok(render!(errors::not_authorized_html(
            &(&mut conn, &rockets).to_context(),
            i18n!(
                rockets.intl.catalog,
                "You are not allowed to edit this blog."
            )
        )))
    }
}

/// Returns true if the media is owned by `user` and is a picture
fn check_media(conn: &mut Connection, id: i32, user: &User) -> bool {
    if let Ok(media) = Media::get(conn, id) {
        media.owner_id == user.id && media.category() == MediaCategory::Image
    } else {
        false
    }
}

#[put("/~/<name>/edit", data = "<form>")]
pub async fn update(
    name: &str,
    form: Form<EditForm>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> RespondOrRedirect {
    let intl = &rockets.intl.catalog;
    let mut blog = Blog::find_by_fqn(&mut conn, name).await.expect("blog::update: blog not found");
    if !rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&mut conn, &blog).ok())
        .unwrap_or(false)
    {
        // TODO actually return 403 error code
        return render!(errors::not_authorized_html(
            &(&mut conn, &rockets).to_context(),
            i18n!(
                rockets.intl.catalog,
                "You are not allowed to edit this blog."
            )
        ))
        .into();
    }

    let user = rockets
        .user
        .clone()
        .expect("blogs::edit: User was None while it shouldn't");
    form.validate()
        .and_then(|_| {
            if let Some(icon) = form.icon {
                if !check_media(&mut conn, icon, &user) {
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
                if !check_media(&mut conn, banner, &user) {
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

            let authors = blog.list_authors(&mut conn)
                .expect("Couldn't get list of authors");

            blog.title = form.title.clone();
            blog.summary = form.summary.clone();
            blog.summary_html = SafeString::new(
                &utils::md_to_html(
                    &form.summary,
                    None,
                    true,
                    Some(Media::get_media_processor(&mut conn, authors.iter().collect())),
                )
                .0,
            );
            blog.icon_id = form.icon;
            blog.banner_id = form.banner;
            blog.theme = form.theme.clone();
            blog.save(&mut conn).expect("Couldn't save blog changes");
            Ok(Flash::success(
                Redirect::to(uri!(details(name = name, page = _))),
                i18n!(intl, "Your blog information have been updated."),
            ))
        })
        .map_err(|err| {
            let medias = Media::for_user(&mut conn, user.id).expect("Couldn't list media");
            render!(blogs::edit_html(
                &(&mut conn, &rockets).to_context(),
                &blog,
                medias,
                &*form,
                err
            ))
        })
        .unwrap()
        .into()
}

#[get("/~/<name>/outbox")]
pub async fn outbox(name: &str, mut conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let blog = Blog::find_by_fqn(&mut conn, name).await.ok()?;
    blog.outbox(&mut conn).ok()
}
#[allow(unused_variables)]
#[get("/~/<name>/outbox?<page>")]
pub async fn outbox_page(
    name: &str,
    page: Page,
    mut conn: DbConn,
) -> Option<ActivityStream<OrderedCollectionPage>> {
    let blog = Blog::find_by_fqn(&mut conn, name).await.ok()?;
    blog.outbox_page(&mut conn, page.limits()).ok()
}
#[get("/~/<name>/atom.xml")]
pub async fn atom_feed(name: &str, mut conn: DbConn) -> Option<(ContentType, String)> {
    let blog = Blog::find_by_fqn(&mut conn, name).await.ok()?;
    let entries = Post::get_recents_for_blog(&mut conn, &blog, 15).ok()?;
    let uri = Instance::get_local()
        .ok()?
        .compute_box("~", &name, "atom.xml");
    let title = &blog.title;
    let default_updated = &blog.creation_date;
    let feed = super::build_atom_feed(entries, &uri, title, default_updated, &mut conn);
    Some((
        ContentType::new("application", "atom+xml"),
        feed.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::valid_slug;
    use crate::init_rocket;
    use diesel::Connection;
    use plume_common::utils::random_hex;
    use plume_models::{
        blog_authors::{BlogAuthor, NewBlogAuthor},
        blogs::{Blog, NewBlog},
        db_conn::{DbConn, DbPool},
        instance::{Instance, NewInstance},
        post_authors::{NewPostAuthor, PostAuthor},
        posts::{NewPost, Post},
        safe_string::SafeString,
        users::{NewUser, User, AUTH_COOKIE},
        Connection as Conn, CONFIG,
    };
    use rocket::{
        http::{Cookie, SameSite},
        local::blocking::Client,
    };

    #[test]
    fn edit_link_within_post_card() {
        let mut conn = Conn::establish(CONFIG.database_url.as_str()).unwrap();

        plume_models::migrations::run_pending_migrations(&mut conn).expect("Couldn't run migrations");

        Instance::insert(
            &mut conn,
            NewInstance {
                public_domain: "example.org".to_string(),
                name: "Plume".to_string(),
                local: true,
                long_description: SafeString::new(""),
                short_description: SafeString::new(""),
                default_license: "CC-BY-SA".to_string(),
                open_registrations: true,
                short_description_html: String::new(),
                long_description_html: String::new(),
            },
        )
        .unwrap();

        let rocket = init_rocket();
        let client = Client::tracked(rocket).expect("valid rocket instance");
        let dbpool = client.rocket().state::<DbPool>().unwrap();
        let mut conn = DbConn(dbpool.get().unwrap());

        let (_instance, user, blog, post) = create_models(&mut conn);

        let blog_path = uri!(super::activity_details(name = &blog.fqn)).to_string();
        let edit_link = uri!(
            super::super::posts::edit(blog = &blog.fqn, slug = &post.slug)
        )
        .to_string();

        let response = client.get(&blog_path).dispatch();
        let body = response.into_string().unwrap();
        assert!(!body.contains(&edit_link));

        let request = client.get(&blog_path)
            .private_cookie(
                Cookie::build((AUTH_COOKIE, user.id.to_string()))
                    .same_site(SameSite::Lax)
                    .build()
            );
        let response = request.dispatch();
        let body = response.into_string().unwrap();
        assert!(body.contains(&edit_link));
    }

    fn create_models(conn: &mut DbConn) -> (Instance, User, Blog, Post) {
        conn.transaction::<(Instance, User, Blog, Post), diesel::result::Error, _>(|conn| {
            let instance = Instance::get_local().unwrap_or_else(|_| {
                let instance = Instance::insert(
                    conn,
                    NewInstance {
                        default_license: "CC-0-BY-SA".to_string(),
                        local: true,
                        long_description: SafeString::new("Good morning"),
                        long_description_html: "<p>Good morning</p>".to_string(),
                        short_description: SafeString::new("Hello"),
                        short_description_html: "<p>Hello</p>".to_string(),
                        name: random_hex(),
                        open_registrations: true,
                        public_domain: random_hex(),
                    },
                )
                .unwrap();
                Instance::cache_local(conn);
                instance
            });
            let user = NewUser {
                instance_id: instance.id,
                username: random_hex(),
                ap_url: random_hex(),
                inbox_url: random_hex(),
                outbox_url: random_hex(),
                followers_endpoint: random_hex(),
                ..Default::default()
            };
            let user = User::insert(conn, user).unwrap();
            let blog = NewBlog {
                instance_id: instance.id,
                actor_id: random_hex(),
                ap_url: random_hex(),
                inbox_url: random_hex(),
                outbox_url: random_hex(),
                ..Default::default()
            };
            let blog = Blog::insert(conn, blog).unwrap();
            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog.id,
                    author_id: user.id,
                    is_owner: true,
                },
            )
            .unwrap();
            let post = Post::insert(
                conn,
                NewPost {
                    blog_id: blog.id,
                    slug: random_hex()[..8].to_owned(),
                    title: random_hex()[..8].to_owned(),
                    content: SafeString::new(""),
                    published: true,
                    license: "CC-By-SA".to_owned(),
                    ap_url: "".to_owned(),
                    creation_date: None,
                    subtitle: "".to_owned(),
                    source: "".to_owned(),
                    cover_id: None,
                },
            )
            .unwrap();
            PostAuthor::insert(
                conn,
                NewPostAuthor {
                    post_id: post.id,
                    author_id: user.id,
                },
            )
            .unwrap();

            Ok((instance, user, blog, post))
        })
        .unwrap()
    }

    #[test]
    fn test_valid_slug() {
        assert!(valid_slug("Blog Title").is_ok());
        assert!(valid_slug("ブログ タイトル").is_ok());
    }
}
