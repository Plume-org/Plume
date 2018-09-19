use rocket_contrib::Json;
use serde_json;

use plume_models::db_conn::DbConn;
use plume_models::posts::Post;

#[get("/posts/<id>")]
fn get(id: i32, conn: DbConn) -> Json<serde_json::Value> {
    let post = Post::get(&*conn, id).unwrap();
    Json(post.to_json(&*conn))
}
