use activitystreams::{
    collection::{OrderedCollection, OrderedCollectionPage},
    iri_string::{spec::IriSpec, validate::iri_reference},
};
use diesel::SaveChangesDsl;
use rocket::{
    http::ContentType,
    request::LenientForm,
    response::{content::Content, Flash, Redirect},
};
use rocket_i18n::I18n;
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::routes::{errors::ErrorPage, Page, RespondOrRedirect};
use crate::template_utils::{IntoContext, Ructe};
use crate::utils::requires_login;
use plume_common::activity_pub::{ActivityStream, ApRequest, CustomGroup};
use plume_common::utils;
use plume_models::{
    blog_authors::*, blogs::*, db_conn::DbConn, instance::Instance, medias::*, posts::Post,
    safe_string::SafeString, users::User, Connection, PlumeRocket,
};

#[get("/~/<name>?<page>", rank = 2)]
pub fn details(
    name: String,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let blog = Blog::find_by_fqn(&conn, &name)?;
    let posts = Post::blog_page(&conn, &blog, page.limits())?;
    let articles_count = Post::count_for_blog(&conn, &blog)?;
    let authors = &blog.list_authors(&conn)?;

    Ok(render!(blogs::details(
        &(&conn, &rockets).to_context(),
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
    conn: DbConn,
    _ap: ApRequest,
) -> Option<ActivityStream<CustomGroup>> {
    let blog = Blog::find_by_fqn(&conn, &name).ok()?;
    Some(ActivityStream::new(blog.to_activity(&conn).ok()?))
}

#[get("/blogs/new")]
pub fn new(conn: DbConn, rockets: PlumeRocket, _user: User) -> Ructe {
    render!(blogs::new(
        &(&conn, &rockets).to_context(),
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
    if slug.is_empty() || iri_reference::<IriSpec>(&slug).is_err() {
        Err(ValidationError::new("empty_slug"))
    } else {
        Ok(())
    }
}

#[post("/blogs/new", data = "<form>")]
pub fn create(
    form: LenientForm<NewBlogForm>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> RespondOrRedirect {
    let slug = Blog::slug(&form.title);
    let intl = &rockets.intl.catalog;
    let user = rockets.user.clone().unwrap();

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
    };
    if Blog::find_by_fqn(&conn, &slug).is_ok() {
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
        return render!(blogs::new(&(&conn, &rockets).to_context(), &*form, errors)).into();
    }

    let blog = Blog::insert(
        &conn,
        NewBlog::new_local(
            slug.clone(),
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
        &conn,
        NewBlogAuthor {
            blog_id: blog.id,
            author_id: user.id,
            is_owner: true,
        },
    )
    .expect("blog::create: author error");

    Flash::success(
        Redirect::to(uri!(details: name = &slug, page = _)),
        &i18n!(intl, "Your blog was successfully created!"),
    )
    .into()
}

#[post("/~/<name>/delete")]
pub fn delete(name: String, conn: DbConn, rockets: PlumeRocket) -> RespondOrRedirect {
    let blog = Blog::find_by_fqn(&conn, &name).expect("blog::delete: blog not found");

    if rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&conn, &blog).ok())
        .unwrap_or(false)
    {
        blog.delete(&conn).expect("blog::expect: deletion error");
        Flash::success(
            Redirect::to(uri!(super::instance::index)),
            i18n!(rockets.intl.catalog, "Your blog was deleted."),
        )
        .into()
    } else {
        // TODO actually return 403 error code
        render!(errors::not_authorized(
            &(&conn, &rockets).to_context(),
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
pub fn edit(name: String, conn: DbConn, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let blog = Blog::find_by_fqn(&conn, &name)?;
    if rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&conn, &blog).ok())
        .unwrap_or(false)
    {
        let user = rockets
            .user
            .clone()
            .expect("blogs::edit: User was None while it shouldn't");
        let medias = Media::for_user(&conn, user.id).expect("Couldn't list media");
        Ok(render!(blogs::edit(
            &(&conn, &rockets).to_context(),
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
        Ok(render!(errors::not_authorized(
            &(&conn, &rockets).to_context(),
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
    conn: DbConn,
    rockets: PlumeRocket,
) -> RespondOrRedirect {
    let intl = &rockets.intl.catalog;
    let mut blog = Blog::find_by_fqn(&conn, &name).expect("blog::update: blog not found");
    if !rockets
        .user
        .clone()
        .and_then(|u| u.is_author_in(&conn, &blog).ok())
        .unwrap_or(false)
    {
        // TODO actually return 403 error code
        return render!(errors::not_authorized(
            &(&conn, &rockets).to_context(),
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
                if !check_media(&conn, icon, &user) {
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
                if !check_media(&conn, banner, &user) {
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
            blog.theme = form.theme.clone();
            blog.save_changes::<Blog>(&*conn)
                .expect("Couldn't save blog changes");
            Ok(Flash::success(
                Redirect::to(uri!(details: name = name, page = _)),
                i18n!(intl, "Your blog information have been updated."),
            ))
        })
        .map_err(|err| {
            let medias = Media::for_user(&conn, user.id).expect("Couldn't list media");
            render!(blogs::edit(
                &(&conn, &rockets).to_context(),
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
pub fn outbox(name: String, conn: DbConn) -> Option<ActivityStream<OrderedCollection>> {
    let blog = Blog::find_by_fqn(&conn, &name).ok()?;
    blog.outbox(&conn).ok()
}
#[allow(unused_variables)]
#[get("/~/<name>/outbox?<page>")]
pub fn outbox_page(
    name: String,
    page: Page,
    conn: DbConn,
) -> Option<ActivityStream<OrderedCollectionPage>> {
    let blog = Blog::find_by_fqn(&conn, &name).ok()?;
    blog.outbox_page(&conn, page.limits()).ok()
}
#[get("/~/<name>/atom.xml")]
pub fn atom_feed(name: String, conn: DbConn) -> Option<Content<String>> {
    let blog = Blog::find_by_fqn(&conn, &name).ok()?;
    let entries = Post::get_recents_for_blog(&conn, &blog, 15).ok()?;
    let uri = Instance::get_local()
        .ok()?
        .compute_box("~", &name, "atom.xml");
    let title = &blog.title;
    let default_updated = &blog.creation_date;
    let feed = super::build_atom_feed(entries, &uri, title, default_updated, &conn);
    Some(Content(
        ContentType::new("application", "atom+xml"),
        feed.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use std::env::var;

    use super::valid_slug;
    use crate::init_rocket;
    use diesel::Connection;
    use plume_common::utils::{make_fqn, random_hex};
    use plume_models::{
        blog_authors::{BlogAuthor, NewBlogAuthor},
        blogs::{Blog, NewBlog},
        db_conn::{DbConn, DbPool},
        get_rocket_config,
        instance::{Instance, NewInstance},
        post_authors::{NewPostAuthor, PostAuthor},
        posts::{NewPost, Post},
        safe_string::SafeString,
        search::Searcher,
        users::{NewUser, User, AUTH_COOKIE},
        Config, Fqn, SearchTokenizerConfig,
    };
    use rocket::{
        http::{ContentType, Cookie, Cookies, SameSite, Status},
        local::{Client, LocalRequest},
    };

    type Models = (Instance, User, Blog, Post);

    fn setup() -> (Client, Models) {
        dotenv::from_path(".env.test").unwrap();
        let config = Config {
            base_url: var("BASE_URL").unwrap(),
            db_name: "plume",
            db_max_size: None,
            db_min_idle: None,
            signup: Default::default(),
            database_url: var("DATABASE_URL").unwrap(),
            search_index: format!("/tmp/plume_test-{}", random_hex()),
            search_tokenizers: SearchTokenizerConfig::init(),
            rocket: get_rocket_config(),
            logo: Default::default(),
            default_theme: Default::default(),
            media_directory: format!("/tmp/plume_test-{}", random_hex()),
            mail: None,
            ldap: None,
            proxy: None,
        };
        let _ = Searcher::create(&config.search_index, &config.search_tokenizers).unwrap();
        let rocket = init_rocket(&config);
        let client = Client::new(rocket).expect("valid rocket instance");
        let dbpool = client.rocket().state::<DbPool>().unwrap();
        let conn = &DbConn(dbpool.get().unwrap());

        (client, create_models(conn))
    }

    fn teardown((client, (instance, user, _blog, _post)): (&Client, Models)) {
        let rocket = client.rocket();

        let dbpool = rocket.state::<DbPool>().unwrap();
        let conn = &DbConn(dbpool.get().unwrap());

        user.delete(conn).unwrap();
        let _ = diesel::delete(&instance);
    }

    #[test]
    fn edit_link_within_post_card() {
        let (client, (instance, user, blog, post)) = setup();

        let blog_path = uri!(super::activity_details: name = &blog.fqn.to_string()).to_string();
        let edit_link = uri!(
            super::super::posts::edit: blog = &blog.fqn.to_string(),
            slug = &post.slug
        )
        .to_string();

        let mut response = client.get(&blog_path).dispatch();
        let body = response.body_string().unwrap();
        let body_not_contain_edit_link = !body.contains(&edit_link);

        let request = client.get(&blog_path);
        login(&request, &user);
        let mut response = request.dispatch();
        let body = response.body_string().unwrap();
        eprintln!("{:?}", &blog.fqn);
        let body_contains_edit_lnk = body.contains(&edit_link);

        teardown((&client, (instance, user, blog, post)));

        assert!(body_not_contain_edit_link);
        assert!(body_contains_edit_lnk);
    }

    fn create_models(conn: &DbConn) -> Models {
        Instance::find_by_domain(conn, "example.org").unwrap_or_else(|_| {
            Instance::insert(
                conn,
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
            .unwrap()
        });

        conn.transaction::<(Instance, User, Blog, Post), diesel::result::Error, _>(|| {
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
                fqn: random_hex(),
                ..Default::default()
            };
            let user = User::insert(conn, user).unwrap();
            let title = random_hex();
            let blog = NewBlog {
                instance_id: instance.id,
                fqn: Fqn::new_local(make_fqn(&title)).unwrap(),
                title,
                actor_id: random_hex(),
                ap_url: random_hex(),
                inbox_url: random_hex(),
                outbox_url: random_hex(),
                summary: Default::default(),
                summary_html: Default::default(),
                public_key: Default::default(),
                private_key: Default::default(),
                icon_id: Default::default(),
                banner_id: Default::default(),
                theme: Default::default(),
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

    fn login(request: &LocalRequest, user: &User) {
        request.inner().guard::<Cookies>().unwrap().add_private(
            Cookie::build(AUTH_COOKIE, user.id.to_string())
                .same_site(SameSite::Lax)
                .finish(),
        );
    }

    #[test]
    fn test_valid_slug() {
        assert!(valid_slug("Blog Title").is_ok());
        assert!(valid_slug("ブログ タイトル").is_ok());
    }

    #[test]
    fn create_blog_with_same_title_twice() {
        let (client, (instance, user, blog, post)) = setup();

        let new_path = uri!(super::new).to_string();
        let request = client.get(new_path);
        login(&request, &user);
        let mut response = request.dispatch();
        let body = response.body_string().unwrap();
        let prefix = r#"<input type="hidden" name="csrf-token" value=""#;
        let pos = body.find(prefix).unwrap();
        let token = body[pos + prefix.len()..pos + prefix.len() + 123].to_string();

        let create_path = uri!(super::create).to_string();
        let response = client
            .post(&create_path)
            .body(format!("title=My%20Blog&csrf-token={}", &token))
            .header(ContentType::Form)
            .dispatch();
        let first_attempt = response;

        let response = client
            .post(&create_path)
            .body(format!("title=My%20Blog&csrf-token={}", &token))
            .header(ContentType::Form)
            .dispatch();
        let second_attempt = response;

        teardown((&client, (instance, user, blog, post)));

        assert_eq!(first_attempt.status(), Status::SeeOther);
        assert_eq!(second_attempt.status(), Status::SeeOther);
    }
}
