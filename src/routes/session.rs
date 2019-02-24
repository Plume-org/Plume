use lettre::EmailTransport;
use lettre_email::EmailBuilder;
use rocket::{
    State,
    http::{Cookie, Cookies, SameSite, uri::Uri},
    response::Redirect,
    request::{LenientForm, FlashMessage, Form}
};
use rocket::http::ext::IntoOwned;
use rocket_i18n::I18n;
use std::{borrow::Cow, env, sync::{Arc, Mutex}, time::Instant};
use validator::{Validate, ValidationError, ValidationErrors};
use template_utils::Ructe;

use plume_models::{
    BASE_URL, Error,
    db_conn::DbConn,
    users::{User, AUTH_COOKIE}
};
use mail::Mailer;
use routes::errors::ErrorPage;

#[get("/login?<m>")]
pub fn new(user: Option<User>, conn: DbConn, m: Option<String>, intl: I18n) -> Ructe {
    render!(session::login(
        &(&*conn, &intl.catalog, user),
        m,
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
        .or_else(|_| User::find_local(&*conn, &form.email_or_name));
    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e
    };

    let user_id = if let Ok(user) = user {
        if !user.auth(&form.password) {
            let mut err = ValidationError::new("invalid_login");
            err.message = Some(Cow::from("Invalid username or password"));
            errors.add("email_or_name", err);
            String::new()
        } else {
            user.id.to_string()
        }
    } else {
        // Fake password verification, only to avoid different login times
        // that could be used to see if an email adress is registered or not
        User::get(&*conn, 1).map(|u| u.auth(&form.password)).expect("No user is registered");

        let mut err = ValidationError::new("invalid_login");
        err.message = Some(Cow::from("Invalid username or password"));
        errors.add("email_or_name", err);
        String::new()
    };

    if errors.is_empty() {
        cookies.add_private(Cookie::build(AUTH_COOKIE, user_id)
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

pub struct ResetRequest {
    pub mail: String,
    pub id: String,
    pub creation_date: Instant,
}

#[get("/password-reset")]
pub fn password_reset_request_form(conn: DbConn, intl: I18n) -> Ructe {
    render!(session::password_reset_request(
        &(&*conn, &intl.catalog, None),
        &ResetForm::default(),
        ValidationErrors::default()
    ))
}

#[derive(FromForm, Validate, Default)]
pub struct ResetForm {
    #[validate(email)]
    pub email: String,
}

#[post("/password-reset", data = "<form>")]
pub fn password_reset_request(
    conn: DbConn,
    intl: I18n,
    mail: State<Arc<Mutex<Mailer>>>,
    form: Form<ResetForm>,
    requests: State<Arc<Mutex<Vec<ResetRequest>>>>
) -> Ructe {
    if User::find_by_email(&*conn, &form.email).is_ok() {
        let id = plume_common::utils::random_hex();
        {
            let mut requests = requests.lock().unwrap();
            requests.push(ResetRequest {
                mail: form.email.clone(),
                id: id.clone(),
                creation_date: Instant::now(),
            });
        }
        let link = format!("https://{}/password-reset/{}", *BASE_URL, id);
        let message = EmailBuilder::new()
            .from(env::var("MAIL_ADDRESS")
                .or_else(|_| Ok(format!("{}@{}", env::var("MAIL_USER")?, env::var("MAIL_SERVER")?)) as Result<_, env::VarError>)
                .expect("Mail server is not correctly configured"))
            .to(form.email.clone())
            .subject(i18n!(intl.catalog, "Password reset"))
            .text(i18n!(intl.catalog, "Here is the link to reset your password: {0}"; link))
            .build()
            .expect("Couldn't build password reset mail");
        match *mail.lock().unwrap() {
            Some(ref mut mail) => { mail.send(&message).map_err(|_| eprintln!("Couldn't send password reset mail")).ok(); }
            None => {}
        }
    }
    render!(session::password_reset_request_ok(
        &(&*conn, &intl.catalog, None)
    ))
}

#[get("/password-reset/<token>")]
pub fn password_reset_form(conn: DbConn, intl: I18n, token: String, requests: State<Arc<Mutex<Vec<ResetRequest>>>>) -> Result<Ructe, ErrorPage> {
    requests.lock().unwrap().iter().find(|x| x.id == token.clone()).ok_or(Error::NotFound)?;
    Ok(render!(session::password_reset(
        &(&*conn, &intl.catalog, None),
        &NewPasswordForm::default(),
        ValidationErrors::default()
    )))
}

#[derive(FromForm, Default, Validate)]
#[validate(
    schema(
        function = "passwords_match",
        skip_on_field_errors = "false",
        message = "Passwords are not matching"
    )
)]
pub struct NewPasswordForm {
    pub password: String,
    pub password_confirmation: String,
}

fn passwords_match(form: &NewPasswordForm) -> Result<(), ValidationError> {
    if form.password != form.password_confirmation {
        Err(ValidationError::new("password_match"))
    } else {
        Ok(())
    }
}

#[post("/password-reset/<token>", data = "<form>")]
pub fn password_reset(
    conn: DbConn,
    intl: I18n,
    token: String,
    requests: State<Arc<Mutex<Vec<ResetRequest>>>>,
    form: Form<NewPasswordForm>
) -> Result<Redirect, ErrorPage> {
    let requests = requests.lock().unwrap();
    let req = requests.iter().find(|x| x.id == token.clone()).ok_or(Error::NotFound)?;
    if req.creation_date.elapsed().as_secs() < 60 * 15 { // Reset link is only valid for 15 minutes
        let user = User::find_by_email(&*conn, &req.mail)?;
        user.reset_password(&*conn, &form.password)?;
        Ok(Redirect::to(uri!(new: m = i18n!(intl.catalog, "Your password was successfully reset."))))
    } else {
        Ok(Redirect::to(uri!(new: m = i18n!(intl.catalog, "Sorry, but the link expired. Try again"))))
    }
}
