use rocket_contrib::Json;
use serde_json;

use plume_common::utils::random_hex;
use plume_models::{
    apps::App,
    api_tokens::*,
    db_conn::DbConn,
    users::User,
};

#[derive(FromForm)]
struct OAuthRequest {
    client_id: String,
    client_secret: String,
    password: String,
    username: String,
    scopes: String,
}

#[get("/oauth2?<query>")]
fn oauth(query: OAuthRequest, conn: DbConn) -> Json<serde_json::Value> {
    let app = App::find_by_client_id(&*conn, query.client_id).expect("OAuth request from unknown client");
    if app.client_secret == query.client_secret {
        if let Some(user) = User::find_local(&*conn, query.username) {
            if user.auth(query.password) {
                let token = ApiToken::insert(&*conn, NewApiToken {
                    app_id: app.id,
                    user_id: user.id,
                    value: random_hex(),
                    scopes: query.scopes,
                });
                Json(json!({
                    "token": token.value
                }))
            } else {
                Json(json!({
                    "error": "Wrong password"
                }))
            }
        } else {
            Json(json!({
                "error": "Unknown user"
            }))
        }
    } else {
        Json(json!({
            "error": "Invalid client_secret"
        }))
    }
}

pub mod apps;
pub mod authorization;
pub mod posts;
