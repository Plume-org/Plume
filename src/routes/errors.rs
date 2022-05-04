use crate::template_utils::{IntoContext, Ructe};
use plume_models::{db_conn::DbConn, Error, PlumeRocket};
use rocket::{
    http::Status,
    response::{self, Responder},
    Request,
};
use tracing::warn;

#[derive(Debug)]
pub struct ErrorPage(Error);

impl From<Error> for ErrorPage {
    fn from(err: Error) -> ErrorPage {
        ErrorPage(err)
    }
}

impl<'r> Responder<'r> for ErrorPage {
    fn respond_to(self, _req: &Request<'_>) -> response::Result<'r> {
        warn!("{:?}", self.0);

        match self.0 {
            Error::NotFound | Error::Unauthorized | Error::Db(diesel::result::Error::NotFound) => {
                Err(Status::NotFound)
            }
            _ => Err(Status::InternalServerError),
        }
    }
}

#[catch(404)]
pub fn not_found(req: &Request<'_>) -> Ructe {
    let conn = req.guard::<DbConn>().unwrap();
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::not_found(&(&conn, &rockets).to_context()))
}

#[catch(422)]
pub fn unprocessable_entity(req: &Request<'_>) -> Ructe {
    let conn = req.guard::<DbConn>().unwrap();
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::unprocessable_entity(
        &(&conn, &rockets).to_context()
    ))
}

#[catch(500)]
pub fn server_error(req: &Request<'_>) -> Ructe {
    let conn = req.guard::<DbConn>().unwrap();
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::server_error(&(&conn, &rockets).to_context()))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(target: Option<String>, conn: DbConn, rockets: PlumeRocket) -> Ructe {
    if let Some(uri) = target {
        warn!("Csrf violation while accessing \"{}\"", uri)
    }
    render!(errors::csrf(&(&conn, &rockets).to_context()))
}
