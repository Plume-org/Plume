#[macro_use]
extern crate rocket;
use rocket::time;

use base64::{engine::general_purpose, Engine as _};
const COOKIE_NAME: &str = "foobar";
const COOKIE_LEN: usize = 64;

fn client(lifetime: Option<time::Duration>) -> rocket::local::blocking::Client {
    rocket::local::blocking::Client::tracked(rocket(lifetime)).unwrap()
}

fn rocket(lifetime: Option<time::Duration>) -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .attach(rocket_csrf::Fairing::new(
            rocket_csrf::CsrfConfig::default()
                .with_cookie_name(COOKIE_NAME)
                .with_cookie_len(COOKIE_LEN)
                .with_lifetime(lifetime),
        ))
        .mount("/", routes![index])
}

#[get("/")]
fn index() {}

#[test]
fn add_csrf_token_to_cookies() {
    general_purpose::STANDARD
        .decode(
            client(Some(time::Duration::days(5)))
                .get("/")
                .dispatch()
                .cookies()
                .iter()
                .find(|cookie| cookie.name() == COOKIE_NAME)
                .unwrap()
                .value(),
        )
        .unwrap();
}

fn process_request(request: &rocket::Request) -> Result<(), &'static str> {
    if request.headers().get_one("X-CSRF-Token").is_none() {
        return Err("Request lacks X-CSRF-Token");
    }
    Ok(())
}

#[test]
fn add_csrf_token_to_cookies_headers_lifetime() {
    let client = client(None);

    // Make a request to the route
    let request = client.get("/");

    match process_request(&request) {
        Err(err) => assert_eq!(err, "Request lacks X-CSRF-Token"),
        Ok(_) => panic!("Expected an error, but got Ok"),
    }

    // Set the X-CSRF-Token header in the request
    let request = client.get("/").header(rocket::http::Header::new(
        "X-CSRF-Token",
        "csrf-token-value",
    ));

    match process_request(&request) {
        Ok(_) => {}
        Err(_) => panic!("Unexpected error"),
    }

    let response = client.get("/").dispatch();

    // Check if the CSRF token cookie exists
    let csrf_cookie = response
        .cookies()
        .iter()
        .find(|cookie| cookie.name() == COOKIE_NAME);

    assert!(csrf_cookie.is_some(), "CSRF token cookie should exist");

    // Get the CSRF token cookie
    let csrf_cookie = csrf_cookie.unwrap();

    // Check if the expiration time is set to None
    assert!(
        csrf_cookie.expires().is_none(),
        "CSRF token cookie should have no expiration"
    );

    // Optionally, you can further inspect other properties of the CSRF token cookie if needed.
    assert_eq!(csrf_cookie.path(), Some("/"));
    // Add more assertions as necessary
}
