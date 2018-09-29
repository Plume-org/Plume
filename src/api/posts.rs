use canapi::Provider;
use diesel::PgConnection;
use rocket::http::uri::Origin;
use rocket_contrib::Json;
use serde_json;
use serde_qs;

use plume_api::posts::PostEndpoint;
use plume_models::db_conn::DbConn;
use plume_models::posts::Post;

#[get("/posts/<id>")]
fn get(id: i32, conn: DbConn) -> Json<serde_json::Value> {
    let post = <Post as Provider<PgConnection>>::get(&*conn, id).ok();
    Json(json!(post))
}

#[get("/posts")]
fn list(conn: DbConn, uri: &Origin) -> Json<serde_json::Value> {
    let query: PostEndpoint = serde_qs::from_str(uri.query().unwrap_or("")).expect("Invalid query string");
    let post = <Post as Provider<PgConnection>>::list(&*conn, query);
    Json(json!(post))
}
