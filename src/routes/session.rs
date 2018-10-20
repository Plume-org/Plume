use rocket::{
    http::{Cookie, Cookies, SameSite, uri::Uri},
    response::Redirect,
    request::{LenientForm,FlashMessage}
};
use rocket_contrib::Template;
use rocket::http::ext::IntoOwned;
use std::borrow::Cow;
use validator::{Validate, ValidationError, ValidationErrors};

use plume_models::{
    db_conn::DbConn,
    users::{User, AUTH_COOKIE}
};

#[get("/login")]
fn new(user: Option<User>, conn: DbConn) -> Template {
    Template::render("session/login", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "errors": null,
        "form": null
    }))
}

#[derive(FromForm)]
struct Message {
	m: String
}

#[get("/login?<message>")]
fn new_message(user: Option<User>, message: Message, conn: DbConn) -> Template {
    Template::render("session/login", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "message": message.m,
        "errors": null,
        "form": null
    }))
}


#[derive(FromForm, Validate, Serialize)]
struct LoginForm {
    #[validate(length(min = "1", message = "We need an email or a username to identify you"))]
    email_or_name: String,
    #[validate(length(min = "1", message = "Your password can't be empty"))]
    password: String
}

#[post("/login", data = "<data>")]
fn create(conn: DbConn, data: LenientForm<LoginForm>, flash: Option<FlashMessage>, mut cookies: Cookies) -> Result<Redirect, Template> {
    let form = data.get();
    let user = User::find_by_email(&*conn, form.email_or_name.to_string())
        .or_else(|| User::find_local(&*conn, form.email_or_name.to_string()));
    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };
    
    if let Some(user) = user.clone() {
        if !user.auth(form.password.clone()) {
            let mut err = ValidationError::new("invalid_login");
            err.message = Some(Cow::from("Invalid username or password"));
            errors.add("email_or_name", err)
        }
    } else {
        // Fake password verification, only to avoid different login times
        // that could be used to see if an email adress is registered or not
        User::get(&*conn, 1).map(|u| u.auth(form.password.clone()));

        let mut err = ValidationError::new("invalid_login");
        err.message = Some(Cow::from("Invalid username or password"));
        errors.add("email_or_name", err)
    }

    if errors.is_empty() {
        cookies.add_private(Cookie::build(AUTH_COOKIE, user.unwrap().id.to_string())
                                            .same_site(SameSite::Lax)
                                            .finish());

        let destination = flash
            .and_then(|f| if f.name() == "callback" {
                Some(f.msg().to_owned())
            } else {
                None
            })
            .unwrap_or("/".to_owned());

        let uri = Uri::parse(&destination)
            .map(|x| x.into_owned())
            .map_err(|_| {
            Template::render("session/login", json!({
                "account": null,
                "errors": errors.inner(),
                "form": form
            }))
        })?;

        Ok(Redirect::to(uri))
    } else {
        println!("{:?}", errors);
        Err(Template::render("session/login", json!({
            "account": null,
            "errors": errors.inner(),
            "form": form
        })))
    }
}

#[get("/logout")]
fn delete(mut cookies: Cookies) -> Redirect {
    cookies.get_private(AUTH_COOKIE).map(|cookie| cookies.remove_private(cookie));
    Redirect::to("/")
}
