use canapi::Provider;
use rocket::http::uri::Origin;
use rocket_contrib::json::Json;
use serde_json;
use serde_qs;

use plume_api::posts::PostEndpoint;
use plume_models::{
    Connection,
    db_conn::DbConn,
    posts::Post,
};
use api::authorization::*;

#[get("/posts/<id>")]
pub fn get(id: i32, conn: DbConn, auth: Option<Authorization<Read, Post>>) -> Json<serde_json::Value> {
    let post = <Post as Provider<(&Connection, Option<i32>)>>::get(&(&*conn, auth.map(|a| a.0.user_id)), id).ok();
    Json(json!(post))
}

#[get("/posts")]
pub fn list(conn: DbConn, uri: &Origin, auth: Option<Authorization<Read, Post>>) -> Json<serde_json::Value> {
    let query: PostEndpoint = serde_qs::from_str(uri.query().unwrap_or("")).expect("api::list: invalid query error");
    let post = <Post as Provider<(&Connection, Option<i32>)>>::list(&(&*conn, auth.map(|a| a.0.user_id)), query);
    Json(json!(post))
}
