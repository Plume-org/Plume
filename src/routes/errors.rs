use rocket_contrib::Template;
use rocket::Request;
use rocket::request::FromRequest;
use models::users::User;

#[catch(404)]
fn not_found(req: &Request) -> Template {
    let user = User::from_request(req).succeeded();
    Template::render("errors/404", json!({
        "error_message": "Page not found",
        "account": user
    }))
}

#[catch(500)]
fn server_error(req: &Request) -> Template {
    let user = User::from_request(req).succeeded();
    Template::render("errors/500", json!({
        "error_message": "Server error",
        "account": user
    }))
}
