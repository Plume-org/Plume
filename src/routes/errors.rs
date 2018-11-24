use rocket_contrib::templates::Template;
use rocket::Request;
use rocket::request::FromRequest;
use plume_models::db_conn::DbConn;
use plume_models::users::User;

#[catch(404)]
pub fn not_found(req: &Request) -> Template {
    let conn = req.guard::<DbConn>().succeeded();
    let user = User::from_request(req).succeeded();
    Template::render("errors/404", json!({
        "error_message": "Page not found",
        "account": user.and_then(|u| conn.map(|conn| u.to_json(&*conn)))
    }))
}

#[catch(500)]
pub fn server_error(req: &Request) -> Template {
    let conn = req.guard::<DbConn>().succeeded();
    let user = User::from_request(req).succeeded();
    Template::render("errors/500", json!({
        "error_message": "Server error",
        "account": user.and_then(|u| conn.map(|conn| u.to_json(&*conn)))
    }))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(target: Option<String>) -> Template {
    if let Some(uri) = target {
        eprintln!("Csrf violation while acceding \"{}\"", uri)
    }
    Template::render("errors/csrf", json!({
        "error_message":""
    }))
}
