use lettre::Transport;
use rocket::http::ext::IntoOwned;
use rocket::{
    http::{uri::Uri, Cookie, Cookies, SameSite},
    request::{FlashMessage, Form, LenientForm},
    response::Redirect,
    State,
};
use rocket_i18n::I18n;
use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
    time::Instant,
};
use template_utils::Ructe;
use validator::{Validate, ValidationError, ValidationErrors};

use mail::{build_mail, Mailer};
use plume_models::{
    db_conn::DbConn,
    users::{User, AUTH_COOKIE},
    Error, BASE_URL,
};
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

#[derive(Default, FromForm, Validate)]
pub struct LoginForm {
    #[validate(length(min = "1", message = "We need an email or a username to identify you"))]
    pub email_or_name: String,
    #[validate(length(min = "1", message = "Your password can't be empty"))]
    pub password: String,
}

#[post("/login", data = "<form>")]
pub fn create(
    conn: DbConn,
    form: LenientForm<LoginForm>,
    flash: Option<FlashMessage>,
    mut cookies: Cookies,
    intl: I18n,
) -> Result<Redirect, Ructe> {
    let user = User::find_by_email(&*conn, &form.email_or_name)
        .or_else(|_| User::find_by_fqn(&*conn, &form.email_or_name));
    let mut errors = match form.validate() {
        Ok(_) => ValidationErrors::new(),
        Err(e) => e,
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
        User::get(&*conn, 1)
            .map(|u| u.auth(&form.password))
            .expect("No user is registered");

        let mut err = ValidationError::new("invalid_login");
        err.message = Some(Cow::from("Invalid username or password"));
        errors.add("email_or_name", err);
        String::new()
    };

    if errors.is_empty() {
        cookies.add_private(
            Cookie::build(AUTH_COOKIE, user_id)
                .same_site(SameSite::Lax)
                .finish(),
        );
        let destination = flash
            .and_then(|f| {
                if f.name() == "callback" {
                    Some(f.msg().to_owned())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "/".to_owned());

        let uri = Uri::parse(&destination)
            .map(|x| x.into_owned())
            .map_err(|_| {
                render!(session::login(
                    &(&*conn, &intl.catalog, None),
                    None,
                    &*form,
                    errors
                ))
            })?;

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

#[derive(Clone)]
pub struct ResetRequest {
    pub mail: String,
    pub id: String,
    pub creation_date: Instant,
}

impl PartialEq for ResetRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
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
    requests: State<Arc<Mutex<Vec<ResetRequest>>>>,
) -> Ructe {
    let mut requests = requests.lock().unwrap();
    // Remove outdated requests (more than 1 day old) to avoid the list to grow too much
    requests.retain(|r| r.creation_date.elapsed().as_secs() < 24 * 60 * 60);

    if User::find_by_email(&*conn, &form.email).is_ok()
        && !requests.iter().any(|x| x.mail == form.email.clone())
    {
        let id = plume_common::utils::random_hex();

        requests.push(ResetRequest {
            mail: form.email.clone(),
            id: id.clone(),
            creation_date: Instant::now(),
        });

        let link = format!("https://{}/password-reset/{}", *BASE_URL, id);
        if let Some(message) = build_mail(
            form.email.clone(),
            i18n!(intl.catalog, "Password reset"),
            i18n!(intl.catalog, "Here is the link to reset your password: {0}"; link),
        ) {
            if let Some(ref mut mail) = *mail.lock().unwrap() {
                mail.send(message.into())
                    .map_err(|_| eprintln!("Couldn't send password reset mail"))
                    .ok();
            }
        }
    }
    render!(session::password_reset_request_ok(&(
        &*conn,
        &intl.catalog,
        None
    )))
}

#[get("/password-reset/<token>")]
pub fn password_reset_form(
    conn: DbConn,
    intl: I18n,
    token: String,
    requests: State<Arc<Mutex<Vec<ResetRequest>>>>,
) -> Result<Ructe, ErrorPage> {
    requests
        .lock()
        .unwrap()
        .iter()
        .find(|x| x.id == token.clone())
        .ok_or(Error::NotFound)?;
    Ok(render!(session::password_reset(
        &(&*conn, &intl.catalog, None),
        &NewPasswordForm::default(),
        ValidationErrors::default()
    )))
}

#[derive(FromForm, Default, Validate)]
#[validate(schema(
    function = "passwords_match",
    skip_on_field_errors = "false",
    message = "Passwords are not matching"
))]
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
    form: Form<NewPasswordForm>,
) -> Result<Redirect, Ructe> {
    form.validate()
        .and_then(|_| {
            let mut requests = requests.lock().unwrap();
            let req = requests
                .iter()
                .find(|x| x.id == token.clone())
                .ok_or_else(|| to_validation(0))?
                .clone();
            if req.creation_date.elapsed().as_secs() < 60 * 60 * 2 {
                // Reset link is only valid for 2 hours
                requests.retain(|r| *r != req);
                let user = User::find_by_email(&*conn, &req.mail).map_err(to_validation)?;
                user.reset_password(&*conn, &form.password).ok();
                Ok(Redirect::to(uri!(
                    new: m = i18n!(intl.catalog, "Your password was successfully reset.")
                )))
            } else {
                Ok(Redirect::to(uri!(
                    new: m = i18n!(intl.catalog, "Sorry, but the link expired. Try again")
                )))
            }
        })
        .map_err(|err| {
            render!(session::password_reset(
                &(&*conn, &intl.catalog, None),
                &form,
                err
            ))
        })
}

fn to_validation<T>(_: T) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    errors.add(
        "",
        ValidationError {
            code: Cow::from("server_error"),
            message: Some(Cow::from("An unknown error occured")),
            params: std::collections::HashMap::new(),
        },
    );
    errors
}
