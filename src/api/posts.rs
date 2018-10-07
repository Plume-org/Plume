use canapi::Provider;
use rocket::http::uri::Origin;
use rocket_contrib::json::Json;
use scheduled_thread_pool::ScheduledThreadPool;
use serde_json;
use serde_qs;

use plume_api::posts::PostEndpoint;
use plume_models::{
    Connection,
    db_conn::DbConn,
    posts::Post,
    search::Searcher as UnmanagedSearcher,
};
use api::authorization::*;
use {Searcher, Worker};

#[get("/posts/<id>")]
pub fn get(id: i32, conn: DbConn, worker: Worker, auth: Option<Authorization<Read, Post>>, search: Searcher) -> Json<serde_json::Value> {
    let post = <Post as Provider<(&Connection, &ScheduledThreadPool, &UnmanagedSearcher, Option<i32>)>>
        ::get(&(&*conn, &worker, &search, auth.map(|a| a.0.user_id)), id).ok();
    Json(json!(post))
}

#[get("/posts")]
pub fn list(conn: DbConn, uri: &Origin, worker: Worker, auth: Option<Authorization<Read, Post>>, search: Searcher) -> Json<serde_json::Value> {
    let query: PostEndpoint = serde_qs::from_str(uri.query().unwrap_or("")).expect("api::list: invalid query error");
    let post = <Post as Provider<(&Connection, &ScheduledThreadPool, &UnmanagedSearcher, Option<i32>)>>
        ::list(&(&*conn, &worker, &search, auth.map(|a| a.0.user_id)), query);
    Json(json!(post))
}

#[post("/posts", data = "<payload>")]
pub fn create(conn: DbConn, payload: Json<PostEndpoint>, worker: Worker, auth: Authorization<Write, Post>, search: Searcher) -> Json<serde_json::Value> {
    let new_post = <Post as Provider<(&Connection, &ScheduledThreadPool, &UnmanagedSearcher, Option<i32>)>>
        ::create(&(&*conn, &worker, &search, Some(auth.0.user_id)), (*payload).clone());
    Json(new_post.map(|p| json!(p)).unwrap_or(json!({
        "error": "Invalid data, couldn't create new post"
    })))
}
