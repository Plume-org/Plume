use rocket::request::FlashMessage;
use plume_models::users::User;
use plume_models::{db_conn::DbConn, Error};
use rocket::{
    request::FromRequest,
    response::{self, Responder},
    Request,
};
use rocket_i18n::I18n;
use template_utils::Ructe;

#[derive(Debug)]
pub struct ErrorPage(Error);

impl From<Error> for ErrorPage {
    fn from(err: Error) -> ErrorPage {
        ErrorPage(err)
    }
}

impl<'r> Responder<'r> for ErrorPage {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
        let conn = req.guard::<DbConn>().succeeded();
        let intl = req.guard::<I18n>().succeeded();
        let user = User::from_request(req).succeeded();
        let msg = req.guard::<FlashMessage>().succeeded();

        match self.0 {
            Error::NotFound => render!(errors::not_found(&(
                &*conn.unwrap(),
                &intl.unwrap().catalog,
                user,
                msg
            )))
            .respond_to(req),
            Error::Unauthorized => render!(errors::not_found(&(
                &*conn.unwrap(),
                &intl.unwrap().catalog,
                user,
                msg
            )))
            .respond_to(req),
            _ => render!(errors::not_found(&(
                &*conn.unwrap(),
                &intl.unwrap().catalog,
                user,
                msg
            )))
            .respond_to(req),
        }
    }
}

#[catch(404)]
pub fn not_found(req: &Request) -> Ructe {
    let conn = req.guard::<DbConn>().succeeded();
    let intl = req.guard::<I18n>().succeeded();
    let user = User::from_request(req).succeeded();
    let msg = req.guard::<FlashMessage>().succeeded();
    render!(errors::not_found(&(
        &*conn.unwrap(),
        &intl.unwrap().catalog,
        user,
        msg
    )))
}

#[catch(422)]
pub fn unprocessable_entity(req: &Request) -> Ructe {
    let conn = req.guard::<DbConn>().succeeded();
    let intl = req.guard::<I18n>().succeeded();
    let user = User::from_request(req).succeeded();
    let msg = req.guard::<FlashMessage>().succeeded();
    render!(errors::unprocessable_entity(&(
        &*conn.unwrap(),
        &intl.unwrap().catalog,
        user,
        msg
    )))
}

#[catch(500)]
pub fn server_error(req: &Request) -> Ructe {
    let conn = req.guard::<DbConn>().succeeded();
    let intl = req.guard::<I18n>().succeeded();
    let user = User::from_request(req).succeeded();
    let msg = req.guard::<FlashMessage>().succeeded();
    render!(errors::server_error(&(
        &*conn.unwrap(),
        &intl.unwrap().catalog,
        user,
        msg
    )))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(
    target: Option<String>,
    conn: DbConn,
    intl: I18n,
    user: Option<User>,
    msg: Option<FlashMessage>,
) -> Ructe {
    if let Some(uri) = target {
        eprintln!("Csrf violation while acceding \"{}\"", uri)
    }
    render!(errors::csrf(&(&*conn, &intl.catalog, user, msg)))
}
