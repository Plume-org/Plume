use rocket::{
    http::{Cookie, Cookies, SameSite, uri::Uri},
    response::Redirect,
    request::{LenientForm,FlashMessage}
};
use rocket::http::ext::IntoOwned;
use rocket_i18n::I18n;
use std::borrow::Cow;
use validator::{Validate, ValidationError, ValidationErrors};
use template_utils::Ructe;

use plume_models::{
    db_conn::DbConn,
    users::{User, AUTH_COOKIE}
};

#[get("/login")]
pub fn new(user: Option<User>, conn: DbConn, intl: I18n) -> Ructe {
    render!(session::login(
        &(&*conn, &intl.catalog, user),
        None,
        &LoginForm::default(),
        ValidationErrors::default()
    ))
}

#[get("/login?<m>")]
pub fn new_message(user: Option<User>, m: String, conn: DbConn, intl: I18n) -> Ructe {
    render!(session::login(
        &(&*conn, &intl.catalog, user),
        Some(m),
        &LoginForm::default(),
        ValidationErrors::default()
    ))
}


#[derive(Default, FromForm, Validate, Serialize)]
pub struct LoginForm {
    #[validate(length(min = "1", message = "We need an email or a username to identify you"))]
    pub email_or_name: String,
    #[validate(length(min = "1", message = "Your password can't be empty"))]
    pub password: String
}

#[post("/login", data = "<form>")]
pub fn create(conn: DbConn, form: LenientForm<LoginForm>, flash: Option<FlashMessage>, mut cookies: Cookies, intl: I18n) -> Result<Redirect, Ructe> {
    let user = User::find_by_email(&*conn, &form.email_or_name)
        .or_else(|| User::find_local(&*conn, &form.email_or_name));
    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };

    if let Some(user) = user.clone() {
        if !user.auth(&form.password) {
            let mut err = ValidationError::new("invalid_login");
            err.message = Some(Cow::from("Invalid username or password"));
            errors.add("email_or_name", err)
        }
    } else {
        // Fake password verification, only to avoid different login times
        // that could be used to see if an email adress is registered or not
        User::get(&*conn, 1).map(|u| u.auth(&form.password));

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
            .unwrap_or_else(|| "/".to_owned());

        let uri = Uri::parse(&destination)
            .map(|x| x.into_owned())
           .map_err(|_| render!(session::login(
                &(&*conn, &intl.catalog, None),
                None,
                &*form,
                errors
            )))?;

        Ok(Redirect::to(uri))
    } else {
        Err(render!(session::login(
            &(&*conn, &intl.catalog, None),
            None,
            &*form,
            errors
        )))
    }
}

#[get("/logout")]
pub fn delete(mut cookies: Cookies) -> Redirect {
    if let Some(cookie) = cookies.get_private(AUTH_COOKIE) {
        cookies.remove_private(cookie);
    }
    Redirect::to("/")
}
