use crate::{
    mail::{build_mail, Mailer},
    routes::{errors::ErrorPage, RespondOrRedirect},
    template_utils::{IntoContext, Ructe},
};
use plume_models::{
    db_conn::DbConn, email_signups::EmailSignup, instance::Instance, lettre::Transport, signups,
    Error, PlumeRocket, CONFIG,
};
use rocket::{
    http::Status,
    request::LenientForm,
    response::{Flash, Redirect},
    State,
};
use std::sync::{Arc, Mutex};
use tracing::warn;
use validator::{Validate, ValidationError, ValidationErrors};

#[derive(Default, FromForm, Validate)]
#[validate(schema(
    function = "emails_match",
    skip_on_field_errors = false,
    message = "Emails are not matching"
))]
pub struct EmailSignupForm {
    #[validate(email(message = "Invalid email"))]
    pub email: String,
    #[validate(email(message = "Invalid email"))]
    pub email_confirmation: String,
}

fn emails_match(form: &EmailSignupForm) -> Result<(), ValidationError> {
    if form.email_confirmation == form.email {
        Ok(())
    } else {
        Err(ValidationError::new("emails_match"))
    }
}

#[derive(Default, FromForm, Validate)]
#[validate(schema(
    function = "passwords_match",
    skip_on_field_errors = false,
    message = "Passwords are not matching"
))]
pub struct NewUserForm {
    #[validate(length(min = 1, message = "Username should be at least 1 characters long"))]
    pub username: String,
    #[validate(length(min = 8, message = "Password should be at least 8 characters long"))]
    pub password: String,
    #[validate(length(min = 8, message = "Password should be at least 8 characters long"))]
    pub password_confirmation: String,
    pub email: String,
    pub token: String,
}

pub fn passwords_match(form: &NewUserForm) -> Result<(), ValidationError> {
    if form.password != form.password_confirmation {
        Err(ValidationError::new("password_match"))
    } else {
        Ok(())
    }
}

#[post("/email_signups/new", data = "<form>")]
pub fn create(
    mail: State<'_, Arc<Mutex<Mailer>>>,
    form: LenientForm<EmailSignupForm>,
    conn: DbConn,
    rockets: PlumeRocket,
    _enabled: signups::Email,
) -> Result<RespondOrRedirect, ErrorPage> {
    let registration_open = Instance::get_local()
        .map(|i| i.open_registrations)
        .unwrap_or(true);

    if !registration_open {
        return Ok(Flash::error(
            Redirect::to(uri!(super::user::new)),
            i18n!(
                rockets.intl.catalog,
                "Registrations are closed on this instance."
            ),
        )
        .into()); // Actually, it is an error
    }
    let mut form = form.into_inner();
    form.email = form.email.trim().to_owned();
    if let Err(err) = form.validate() {
        return Ok(render!(email_signups::new(
            &(&conn, &rockets).to_context(),
            registration_open,
            &form,
            err
        ))
        .into());
    }
    let res = EmailSignup::start(&conn, &form.email);
    if let Some(err) = res.as_ref().err() {
        return Ok(match err {
            Error::UserAlreadyExists => {
                // TODO: Notify to admin (and the user?)
                warn!("Registration attempted for existing user: {}. Registraion halted and email sending skipped.", &form.email);
                render!(email_signups::create(&(&conn, &rockets).to_context())).into()
            }
            Error::NotFound => render!(errors::not_found(&(&conn, &rockets).to_context())).into(),
            _ => render!(errors::not_found(&(&conn, &rockets).to_context())).into(), // FIXME
        });
    }
    let token = res.unwrap();
    let url = format!(
        "https://{}{}",
        CONFIG.base_url,
        uri!(show: token = token.to_string())
    );
    let message = build_mail(
        form.email,
        i18n!(rockets.intl.catalog, "User registration"),
        i18n!(rockets.intl.catalog, "Here is the link for registration: {0}"; url),
    )
    .expect("Mail configuration has already been done at ignition process");
    // TODO: Render error page
    if let Some(ref mut mailer) = *mail.lock().unwrap() {
        mailer.send(message.into()).ok(); // TODO: Render error page
    }

    Ok(render!(email_signups::create(&(&conn, &rockets).to_context())).into())
}

#[get("/email_signups/new")]
pub fn created(conn: DbConn, rockets: PlumeRocket, _enabled: signups::Email) -> Ructe {
    render!(email_signups::create(&(&conn, &rockets).to_context()))
}

#[get("/email_signups/<token>")]
pub fn show(
    token: String,
    conn: DbConn,
    rockets: PlumeRocket,
    _enabled: signups::Email,
) -> Result<Ructe, ErrorPage> {
    let signup = EmailSignup::find_by_token(&conn, token.into())?;
    let confirmation = signup.confirm(&conn);
    if let Some(err) = confirmation.err() {
        match err {
            Error::Expired => {
                return Ok(render!(email_signups::new(
                    &(&conn, &rockets).to_context(),
                    Instance::get_local()?.open_registrations,
                    &EmailSignupForm::default(),
                    ValidationErrors::default()
                )))
            } // TODO: Flash and redirect
            Error::NotFound => return Err(Error::NotFound.into()),
            _ => return Err(Error::NotFound.into()), // FIXME
        }
    }

    let form = NewUserForm {
        email: signup.email,
        token: signup.token,
        ..NewUserForm::default()
    };
    Ok(render!(email_signups::edit(
        &(&conn, &rockets).to_context(),
        Instance::get_local()?.open_registrations,
        &form,
        ValidationErrors::default()
    )))
}

#[post("/email_signups/signup", data = "<form>")]
pub fn signup(
    form: LenientForm<NewUserForm>,
    conn: DbConn,
    rockets: PlumeRocket,
    _enabled: signups::Email,
) -> Result<RespondOrRedirect, Status> {
    use RespondOrRedirect::{FlashRedirect, Response};

    let instance = Instance::get_local().map_err(|e| {
        warn!("{:?}", e);
        Status::InternalServerError
    })?;
    if let Some(err) = form.validate().err() {
        return Ok(Response(render!(email_signups::edit(
            &(&conn, &rockets).to_context(),
            instance.open_registrations,
            &form,
            err
        ))));
    }
    let signup = EmailSignup::find_by_token(&conn, form.token.clone().into())
        .map_err(|_| Status::NotFound)?;
    if form.email != signup.email {
        let mut err = ValidationErrors::default();
        err.add("email", ValidationError::new("Email couldn't changed"));
        let form = NewUserForm {
            email: signup.email,
            ..form.into_inner()
        };
        return Ok(Response(render!(email_signups::edit(
            &(&conn, &rockets).to_context(),
            instance.open_registrations,
            &form,
            err
        ))));
    }
    let _user = signup
        .complete(&conn, form.username.clone(), form.password.clone())
        .map_err(|e| {
            warn!("{:?}", e);
            Status::UnprocessableEntity
        })?;
    Ok(FlashRedirect(Flash::success(
        Redirect::to(uri!(super::session::new: m = _)),
        i18n!(
            rockets.intl.catalog,
            "Your account has been created. Now you just need to log in, before you can use it."
        ),
    )))
}
