use crate::template_utils::{IntoContext, Ructe};
use tracing::warn;
use plume_models::{Error, PlumeRocket};
use rocket::{
    response::{self, Responder},
    Request,
};

#[derive(Debug)]
pub struct ErrorPage(Error);

impl From<Error> for ErrorPage {
    fn from(err: Error) -> ErrorPage {
        ErrorPage(err)
    }
}

impl<'r> Responder<'r> for ErrorPage {
    fn respond_to(self, req: &Request<'_>) -> response::Result<'r> {
        let rockets = req.guard::<PlumeRocket>().unwrap();

        match self.0 {
            Error::NotFound => render!(errors::not_found(&rockets.to_context())).respond_to(req),
            Error::Unauthorized => {
                render!(errors::not_found(&rockets.to_context())).respond_to(req)
            }
            _ => render!(errors::not_found(&rockets.to_context())).respond_to(req),
        }
    }
}

#[catch(404)]
pub fn not_found(req: &Request<'_>) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::not_found(&rockets.to_context()))
}

#[catch(422)]
pub fn unprocessable_entity(req: &Request<'_>) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::unprocessable_entity(&rockets.to_context()))
}

#[catch(500)]
pub fn server_error(req: &Request<'_>) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::server_error(&rockets.to_context()))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(target: Option<String>, rockets: PlumeRocket) -> Ructe {
    if let Some(uri) = target {
        warn!("Csrf violation while accessing \"{}\"", uri)
    }
    render!(errors::csrf(&rockets.to_context()))
}
