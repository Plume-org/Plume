#![feature(decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;

use dotenv;
use rocket::form::Form;
use rocket::request::{FlashMessage, FromRequest};
use rocket::response::{Flash, Redirect};
use rocket_csrf_token::CsrfToken;
use rocket_dyn_templates::Template;

#[derive(Serialize)]
struct TemplateContext {
    authenticity_token: String,
    flash: Option<String>,
}

#[derive(FromForm)]
struct Comment {
    authenticity_token: String,
    text: String,
}

struct Authenticated;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Authenticated {
    type Error = std::convert::Infallible;

    async fn from_request(
        _request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        // Authentication logic
        rocket::request::Outcome::Success(Authenticated)
    }
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenv::dotenv().ok();
    rocket::build()
        .attach(rocket_csrf_token::Fairing::new(
            rocket_csrf_token::CsrfConfig::default().with_lifetime(None),
        ))
        .attach(Template::fairing())
        .register("/", catchers![not_authorized])
        .mount("/", routes![index, new, create])
        .launch()
        .await
        .expect("Launch Error");
    Ok(())
}

#[get("/")]
fn index() -> Redirect {
    Redirect::to(uri!(new))
}

#[get("/comments/new")]
fn new(
    csrf_token: CsrfToken,
    flash: Option<FlashMessage>,
    _authenticated: Authenticated,
) -> Template {
    let template_context = TemplateContext {
        authenticity_token: csrf_token.authenticity_token().unwrap().to_string(),
        flash: flash.map(|flash| flash.message().to_string()),
    };

    Template::render("comments/new", &template_context)
}

#[post("/comments", data = "<form>")]
fn create(
    csrf_token: CsrfToken,
    form: Form<Comment>,
    _authenticated: Authenticated,
) -> Flash<Redirect> {
    if let Err(_) = csrf_token.verify(&form.authenticity_token) {
        return Flash::error(Redirect::to(uri!(new)), "Invalid authenticity token");
    }

    Flash::success(
        Redirect::to(uri!(new)),
        format!("Created comment: {:#?}", form.text),
    )
}

#[catch(403)]
fn not_authorized() -> String {
    "403 Forbidden: Unauthorized Access".to_string()
}
