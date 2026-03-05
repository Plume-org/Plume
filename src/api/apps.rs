use rocket::serde::json::Json;

use crate::api::Api;
use plume_api::apps::NewAppData;
use plume_common::utils::random_hex;
use plume_models::{apps::*, db_conn::DbConn};

#[post("/apps", data = "<data>")]
pub fn create(mut conn: DbConn, data: Json<NewAppData>) -> Api<App> {
    let client_id = random_hex();
    let client_secret = random_hex();
    let app = App::insert(
        &mut conn,
        NewApp {
            name: data.name.clone(),
            client_id,
            client_secret,
            redirect_uri: data.redirect_uri.clone(),
            website: data.website.clone(),
        },
    )?;

    Ok(Json(app))
}
