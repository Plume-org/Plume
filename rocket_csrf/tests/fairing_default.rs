#[macro_use]
extern crate rocket;

use base64::{engine::general_purpose, Engine as _};
fn client() -> rocket::local::blocking::Client {
    rocket::local::blocking::Client::tracked(rocket()).unwrap()
}

fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .attach(rocket_csrf::Fairing::default())
        .mount("/", routes![index])
}

#[get("/")]
fn index() {}

#[test]
fn add_csrf_token_to_cookies() {
    general_purpose::STANDARD
        .decode(
            client()
                .get("/")
                .dispatch()
                .cookies()
                .iter()
                .find(|cookie| cookie.name() == "csrf_token")
                .unwrap()
                .value(),
        )
        .unwrap();
}
