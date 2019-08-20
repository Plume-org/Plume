use plume_models::{instance::Instance, Error, PlumeRocket};
use rocket::{
    response::{self, Redirect, Responder},
    Request,
};
use template_utils::{IntoContext, Ructe};

#[derive(Debug)]
pub struct ErrorPage(Error);

impl From<Error> for ErrorPage {
    fn from(err: Error) -> ErrorPage {
        ErrorPage(err)
    }
}

impl<'r> Responder<'r> for ErrorPage {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
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
pub fn not_found(req: &Request) -> Result<Ructe, Redirect> {
    let rockets = req.guard::<PlumeRocket>().unwrap();
    if req
        .uri()
        .segments()
        .next()
        .map(|path| path == "custom_domains")
        .unwrap_or(false)
    {
        let path = req
            .uri()
            .segments()
            .skip(2)
            .collect::<Vec<&str>>()
            .join("/");
        let public_domain = Instance::get_local().unwrap().public_domain;
        Err(Redirect::to(format!("https://{}/{}", public_domain, path)))
    } else {
        Ok(render!(errors::not_found(&rockets.to_context())))
    }
}

#[catch(422)]
pub fn unprocessable_entity(req: &Request) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::unprocessable_entity(&rockets.to_context()))
}

#[catch(500)]
pub fn server_error(req: &Request) -> Ructe {
    let rockets = req.guard::<PlumeRocket>().unwrap();
    render!(errors::server_error(&rockets.to_context()))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(target: Option<String>, rockets: PlumeRocket) -> Ructe {
    if let Some(uri) = target {
        eprintln!("Csrf violation while acceding \"{}\"", uri)
    }
    render!(errors::csrf(&rockets.to_context()))
}
