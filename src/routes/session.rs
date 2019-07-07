use lettre::Transport;
use rocket::http::ext::IntoOwned;
use rocket::{
    http::{uri::Uri, Cookie, Cookies, SameSite},
    request::{Form, LenientForm},
    response::{Flash, Redirect},
    State,
};
use rocket_i18n::I18n;
use routes::RespondOrRedirect;
use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
    time::Instant,
};
use validator::{Validate, ValidationError, ValidationErrors};

use mail::{build_mail, Mailer};
use plume_models::{
    password_reset_requests::*,
    users::{User, AUTH_COOKIE},
    Error, PlumeRocket, CONFIG,
};
use template_utils::{IntoContext, Ructe};

#[get("/login?<m>")]
pub fn new(m: Option<String>, rockets: PlumeRocket) -> Ructe {
    render!(session::login(
        &rockets.to_context(),
        m,
        &LoginForm::default(),
        ValidationErrors::default()
    ))
}

#[derive(Default, FromForm)]
pub struct LoginForm {
    pub email_or_name: String,
    pub password: String,
}

#[post("/login", data = "<form>")]
pub fn create(
    form: LenientForm<LoginForm>,
    rockets: PlumeRocket,
    mut cookies: Cookies,
) -> RespondOrRedirect {
    let conn = &*rockets.conn;

    let user_id = if let Ok(user) = User::connect(&rockets, &form.email_or_name, &form.password) {
        user.id.to_string()
    } else {
        let mut errors = ValidationErrors::new();
        let mut err = ValidationError::new("invalid_login");
        err.message = Some(Cow::from("Invalid username, or password"));
        errors.add("email_or_name", err);
        return render!(session::login(&rockets.to_context(), None, &*form, errors)).into();
    };

    cookies.add_private(
        Cookie::build(AUTH_COOKIE, user_id)
            .same_site(SameSite::Lax)
            .finish(),
    );
    let destination = rockets
        .flash_msg
        .clone()
        .and_then(
            |(name, msg)| {
                if name == "callback" {
                    Some(msg)
                } else {
                    None
                }
            },
        )
        .unwrap_or_else(|| "/".to_owned());

    if let Ok(uri) = Uri::parse(&destination).map(IntoOwned::into_owned) {
        Flash::success(
            Redirect::to(uri),
            i18n!(&rockets.intl.catalog, "You are now connected."),
        )
        .into()
    } else {
        render!(session::login(
            &(conn, &rockets.intl.catalog, None, None),
            None,
            &*form,
            ValidationErrors::new()
        ))
        .into()
    }
}

#[get("/logout")]
pub fn delete(mut cookies: Cookies, intl: I18n) -> Flash<Redirect> {
    if let Some(cookie) = cookies.get_private(AUTH_COOKIE) {
        cookies.remove_private(cookie);
    }
    Flash::success(
        Redirect::to("/"),
        i18n!(intl.catalog, "You are now logged off."),
    )
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
pub fn password_reset_request_form(rockets: PlumeRocket) -> Ructe {
    render!(session::password_reset_request(
        &rockets.to_context(),
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
    mail: State<Arc<Mutex<Mailer>>>,
    form: Form<ResetForm>,
    rockets: PlumeRocket,
) -> Ructe {
    if User::find_by_email(&*rockets.conn, &form.email).is_ok() {
        let token = PasswordResetRequest::insert(&*rockets.conn, &form.email)
            .expect("password_reset_request::insert: error");

        let url = format!("https://{}/password-reset/{}", CONFIG.base_url, token);
        if let Some(message) = build_mail(
            form.email.clone(),
            i18n!(rockets.intl.catalog, "Password reset"),
            i18n!(rockets.intl.catalog, "Here is the link to reset your password: {0}"; url),
        ) {
            if let Some(ref mut mail) = *mail.lock().unwrap() {
                mail.send(message.into())
                    .map_err(|_| eprintln!("Couldn't send password reset email"))
                    .ok();
            }
        }
    }
    render!(session::password_reset_request_ok(&rockets.to_context()))
}

#[get("/password-reset/<token>")]
pub fn password_reset_form(token: String, rockets: PlumeRocket) -> Result<Ructe, Ructe> {
    PasswordResetRequest::find_by_token(&*rockets.conn, &token)
        .map_err(|err| password_reset_error_response(err, &rockets))?;

    Ok(render!(session::password_reset(
        &rockets.to_context(),
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
    token: String,
    form: Form<NewPasswordForm>,
    rockets: PlumeRocket,
) -> Result<Flash<Redirect>, Ructe> {
    form.validate()
        .map_err(|err| render!(session::password_reset(&rockets.to_context(), &form, err)))?;

    PasswordResetRequest::find_and_delete_by_token(&*rockets.conn, &token)
        .and_then(|request| User::find_by_email(&*rockets.conn, &request.email))
        .and_then(|user| user.reset_password(&*rockets.conn, &form.password))
        .map_err(|err| password_reset_error_response(err, &rockets))?;

    Ok(Flash::success(
        Redirect::to(uri!(
            new: m = _
        )),
        i18n!(
            rockets.intl.catalog,
            "Your password was successfully reset."
        ),
    ))
}

fn password_reset_error_response(err: Error, rockets: &PlumeRocket) -> Ructe {
    match err {
        Error::Expired => render!(session::password_reset_request_expired(
            &rockets.to_context()
        )),
        _ => render!(errors::not_found(&rockets.to_context())),
    }
}
