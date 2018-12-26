use rocket::{
    Request,
    request::FromRequest,
    response::{self, Responder},
};
use rocket_i18n::I18n;
use plume_models::{Error, db_conn::DbConn};
use plume_models::users::User;
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

        match self.0 {
            Error::NotFound => render!(errors::not_found(
                &(&*conn.unwrap(), &intl.unwrap().catalog, user)
            )).respond_to(req),
            Error::Unauthorized => render!(errors::not_found(
                &(&*conn.unwrap(), &intl.unwrap().catalog, user)
            )).respond_to(req),
            _ => render!(errors::not_found(
                &(&*conn.unwrap(), &intl.unwrap().catalog, user)
            )).respond_to(req)
        }
    }
}

#[catch(404)]
pub fn not_found(req: &Request) -> Ructe {
    let conn = req.guard::<DbConn>().succeeded();
    let intl = req.guard::<I18n>().succeeded();
    let user = User::from_request(req).succeeded();
    render!(errors::not_found(
        &(&*conn.unwrap(), &intl.unwrap().catalog, user)
    ))
}

#[catch(500)]
pub fn server_error(req: &Request) -> Ructe {
    let conn = req.guard::<DbConn>().succeeded();
    let intl = req.guard::<I18n>().succeeded();
    let user = User::from_request(req).succeeded();
    render!(errors::server_error(
        &(&*conn.unwrap(), &intl.unwrap().catalog, user)
    ))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(target: Option<String>, conn: DbConn, intl: I18n, user: Option<User>) -> Ructe {
    if let Some(uri) = target {
        eprintln!("Csrf violation while acceding \"{}\"", uri)
    }
    render!(errors::csrf(
        &(&*conn, &intl.catalog, user)
    ))
}
