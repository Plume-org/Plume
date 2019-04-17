use canapi::{Error as ApiError, Provider};
use rocket::http::uri::Origin;
use rocket_contrib::json::Json;
use serde_json;
use serde_qs;

use api::authorization::*;
use plume_api::posts::PostEndpoint;
use plume_models::{posts::Post, users::User, PlumeRocket};

#[get("/posts/<id>")]
pub fn get(
    id: i32,
    auth: Option<Authorization<Read, Post>>,
    mut rockets: PlumeRocket,
) -> Json<serde_json::Value> {
    rockets.user = auth.and_then(|a| User::get(&*rockets.conn, a.0.user_id).ok());
    let post = <Post as Provider<PlumeRocket>>::get(&rockets, id).ok();
    Json(json!(post))
}

#[get("/posts")]
pub fn list(
    uri: &Origin,
    auth: Option<Authorization<Read, Post>>,
    mut rockets: PlumeRocket,
) -> Json<serde_json::Value> {
    rockets.user = auth.and_then(|a| User::get(&*rockets.conn, a.0.user_id).ok());
    let query: PostEndpoint =
        serde_qs::from_str(uri.query().unwrap_or("")).expect("api::list: invalid query error");
    let post = <Post as Provider<PlumeRocket>>::list(&rockets, query);
    Json(json!(post))
}

#[post("/posts", data = "<payload>")]
pub fn create(
    auth: Authorization<Write, Post>,
    payload: Json<PostEndpoint>,
    mut rockets: PlumeRocket,
) -> Json<serde_json::Value> {
    rockets.user = User::get(&*rockets.conn, auth.0.user_id).ok();
    let new_post = <Post as Provider<PlumeRocket>>::create(&rockets, (*payload).clone());
    Json(new_post.map(|p| json!(p)).unwrap_or_else(|e| {
        json!({
            "error": "Invalid data, couldn't create new post",
            "details": match e {
                ApiError::Fetch(msg) => msg,
                ApiError::SerDe(msg) => msg,
                ApiError::NotFound(msg) => msg,
                ApiError::Authorization(msg) => msg,
            }
        })
    }))
}
