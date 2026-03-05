#[macro_use]
extern crate rocket;

use bcrypt::verify;
use rand::RngCore;
use rocket::http::Cookie;
use rocket_csrf::CsrfToken;

use base64::{engine::general_purpose, Engine as _};

const COOKIE_NAME: &str = "foobar";
const COOKIE_LEN: usize = 64;

fn client() -> rocket::local::blocking::Client {
    rocket::local::blocking::Client::tracked(rocket()).unwrap()
}

fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .attach(rocket_csrf::Fairing::new(
            rocket_csrf::CsrfConfig::default()
                .with_cookie_name(COOKIE_NAME)
                .with_cookie_len(COOKIE_LEN)
                .with_lifetime(Some(rocket::time::Duration::days(3))),
        ))
        .mount("/", routes![index])
}

#[get("/")]
fn index(csrf_token: CsrfToken) -> String {
    csrf_token.authenticity_token().unwrap().to_string()
}

#[test]
fn respond_with_valid_authenticity_token() {
    let mut raw = [0u8; COOKIE_LEN];
    rand::rng().fill_bytes(&mut raw);

    let encoded = general_purpose::STANDARD.encode(raw);

    let body = client()
        .get("/")
        .private_cookie(Cookie::new(COOKIE_NAME, encoded.to_string()))
        .dispatch()
        .into_string()
        .unwrap();

    assert!(verify(&encoded, &body).unwrap());
}
