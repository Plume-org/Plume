use rocket_contrib::Template;

#[catch(404)]
fn not_found() -> Template {
    Template::render("errors/404", json!({
        "error_message": "Page not found"
    }))
}

#[catch(500)]
fn server_error() -> Template {
    Template::render("errors/500", json!({
        "error_message": "Server error"
    }))
}
