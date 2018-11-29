use rocket::Request;
use rocket::request::FromRequest;
use rocket_i18n::I18n;
use plume_models::db_conn::DbConn;
use plume_models::users::User;
use routes::Ructe;

#[catch(404)]
pub fn not_found(req: &Request) -> Ructe {
    let conn = req.guard::<DbConn>().succeeded();
    let intl = req.guard::<I18n>().succeeded();
    let user = User::from_request(req).succeeded();
    render!(errors::not_found(
        &(&*conn.unwrap(), &intl.unwrap().catalog, user)
    ))
}

#[catch(500)]
pub fn server_error(req: &Request) -> Ructe {
    let conn = req.guard::<DbConn>().succeeded();
    let intl = req.guard::<I18n>().succeeded();
    let user = User::from_request(req).succeeded();
    render!(errors::server_error(
        &(&*conn.unwrap(), &intl.unwrap().catalog, user)
    ))
}

#[post("/csrf-violation?<target>")]
pub fn csrf_violation(target: Option<String>, conn: DbConn, intl: I18n, user: Option<User>) -> Ructe {
    if let Some(uri) = target {
        eprintln!("Csrf violation while acceding \"{}\"", uri)
    }
    render!(errors::csrf(
        &(&*conn, &intl.catalog, user)
    ))
}
