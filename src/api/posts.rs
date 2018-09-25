use canapi::Provider;
use diesel::PgConnection;
use rocket_contrib::Json;
use serde_json;

use plume_api::posts::PostEndpoint;
use plume_models::db_conn::DbConn;
use plume_models::posts::Post;

#[get("/posts/<id>")]
fn get(id: i32, conn: DbConn) -> Json<serde_json::Value> {
    let post = <Post as Provider<PgConnection>>::get(&*conn, id).ok();
    Json(json!(post))
}

// TODO: handle query params
#[get("/posts")]
fn list(conn: DbConn) -> Json<serde_json::Value> {
    let post = <Post as Provider<PgConnection>>::list(&*conn, PostEndpoint::default());
    Json(json!(post))
}
