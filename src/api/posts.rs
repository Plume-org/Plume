use canapi::Provider;
use rocket::http::uri::Origin;
use rocket_contrib::Json;
use serde_json;
use serde_qs;

use plume_api::posts::PostEndpoint;
use plume_models::{
    Connection,
    api_tokens::ApiToken,
    db_conn::DbConn,
    posts::Post,
};

#[get("/posts/<id>")]
fn get(id: i32, conn: DbConn, token: ApiToken) -> Json<serde_json::Value> {
    if token.can_read("posts") {
        let post = <Post as Provider<Connection>>::get(&*conn, id).ok();
        Json(json!(post))
    } else {
        Json(json!({
            "error": "Unauthorized"
        }))
    }
}

#[get("/posts")]
fn list(conn: DbConn, uri: &Origin, token: ApiToken) -> Json<serde_json::Value> {
    if token.can_read("posts") {
        let query: PostEndpoint = serde_qs::from_str(uri.query().unwrap_or("")).expect("api::list: invalid query error");
        let post = <Post as Provider<Connection>>::list(&*conn, query);
        Json(json!(post))
    } else {
        Json(json!({
            "error": "Unauthorized"
        }))
    }}
