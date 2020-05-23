use crate::template_utils::{IntoContext, Ructe};
use plume_models::{Error, PlumeRocket};
use rocket::{
    request::FromRequest,
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

#[rocket::async_trait]
impl<'r> Responder<'r> for ErrorPage {
    async fn respond_to(self, req: &'r Request<'_>) -> response::Result<'r> {
        let rockets = PlumeRocket::from_request(req).await.unwrap();

        match self.0 {
            Error::NotFound => {
                render!(errors::not_found(&rockets.to_context()))
                    .respond_to(req)
                    .await
            }
            Error::Unauthorized => {
                render!(errors::not_found(&rockets.to_context()))
                    .respond_to(req)
                    .await
            }
            _ => {
                render!(errors::not_found(&rockets.to_context()))
                    .respond_to(req)
                    .await
            }
        }
    }
}

#[catch(404)]
pub async fn not_found(req: &Request<'_>) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().await.unwrap();
    render!(errors::not_found(&rockets.to_context()))
}

#[catch(422)]
pub async fn unprocessable_entity(req: &Request<'_>) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().await.unwrap();
    render!(errors::unprocessable_entity(&rockets.to_context()))
}

#[catch(500)]
pub async fn server_error(req: &Request<'_>) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().await.unwrap();
    render!(errors::server_error(&rockets.to_context()))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(target: Option<String>, rockets: PlumeRocket) -> Ructe {
    if let Some(uri) = target {
        eprintln!("Csrf violation while acceding \"{}\"", uri)
    }
    render!(errors::csrf(&rockets.to_context()))
}
