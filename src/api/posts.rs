use canapi::Provider;
use rocket::http::uri::Origin;
use rocket_contrib::Json;
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
fn get(id: i32, conn: DbConn, _auth: Authorization<Read, Post>) -> Json<serde_json::Value> {
    let post = <Post as Provider<Connection>>::get(&*conn, id).ok();
    Json(json!(post))
}

#[get("/posts")]
fn list(conn: DbConn, uri: &Origin, _auth: Authorization<Read, Post>) -> Json<serde_json::Value> {
    let query: PostEndpoint = serde_qs::from_str(uri.query().unwrap_or("")).expect("api::list: invalid query error");
    let post = <Post as Provider<Connection>>::list(&*conn, query);
    Json(json!(post))
}
