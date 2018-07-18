use activitypub::collection::OrderedCollection;
use rocket::{
    request::LenientForm,
    response::{Redirect, Flash}
};
use rocket_contrib::Template;
use serde_json;
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

#[get("/~/<name>", rank = 2)]
fn details(name: String, conn: DbConn, user: Option<User>) -> Template {
    may_fail!(user, Blog::find_by_fqn(&*conn, name), "Requested blog couldn't be found", |blog| {
        let recents = Post::get_recents_for_blog(&*conn, &blog, 5);
        let authors = &blog.list_authors(&*conn);

        Template::render("blogs/details", json!({
            "blog": &blog,
            "account": user,
            "is_author": user.map(|x| x.is_author_in(&*conn, blog.clone())),
            "recents": recents.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "authors": authors.into_iter().map(|u| u.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
            "n_authors": authors.len()
        }))
    })    
}

#[get("/~/<name>", rank = 1)]
fn activity_details(name: String, conn: DbConn, _ap: ApRequest) -> ActivityStream<CustomGroup> {
    let blog = Blog::find_local(&*conn, name).unwrap();
    ActivityStream::new(blog.into_activity(&*conn))
}

#[get("/blogs/new")]
fn new(user: User) -> Template {
    Template::render("blogs/new", json!({
        "account": user,
        "errors": null,
        "form": null
    }))
}

#[get("/blogs/new", rank = 2)]
fn new_auth() -> Flash<Redirect>{
    utils::requires_login("You need to be logged in order to create a new blog", uri!(new))
}

#[derive(FromForm, Validate, Serialize)]
struct NewBlogForm {
    #[validate(custom(function = "valid_slug", message = "Invalid name"))]
    pub title: String
}

fn valid_slug(title: &str) -> Result<(), ValidationError> {
    let slug = utils::make_actor_id(title.to_string());
    if slug.len() == 0 {
        Err(ValidationError::new("empty_slug"))
    } else {
        Ok(())
    }
}

#[post("/blogs/new", data = "<data>")]
fn create(conn: DbConn, data: LenientForm<NewBlogForm>, user: User) -> Result<Redirect, Template> {
    let form = data.get();
    let slug = utils::make_actor_id(form.title.to_string());

    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };
    if let Some(_) = Blog::find_local(&*conn, slug.clone()) {
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
            "account": user,
            "errors": errors.inner(),
            "form": form
        })))
    }
}

#[get("/~/<name>/outbox")]
fn outbox(name: String, conn: DbConn) -> ActivityStream<OrderedCollection> {
    let blog = Blog::find_local(&*conn, name).unwrap();
    blog.outbox(&*conn)
}
