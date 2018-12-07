use canapi::Provider;
use rocket_contrib::json::Json;
use serde_json;

use plume_api::apps::AppEndpoint;
use plume_models::{
    Connection,
    db_conn::DbConn,
    apps::App,
};

#[post("/apps", data = "<data>")]
pub fn create(conn: DbConn, data: Json<AppEndpoint>) -> Json<serde_json::Value> {
    let post = <App as Provider<Connection>>::create(&*conn, (*data).clone()).ok();
    Json(json!(post))
}
