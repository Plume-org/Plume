use rocket::{response::{self, Responder}, request::{Form, Request}};
use rocket_contrib::json::Json;
use serde_json;

use plume_common::utils::random_hex;
use plume_models::{
    Context,
    Error,
    apps::App,
    api_tokens::*,
    db_conn::DbConn,
    users::User,
};
use Searcher;

#[derive(Debug)]
pub struct ApiError(Error);

impl From<Error> for ApiError {
    fn from(err: Error) -> ApiError {
        ApiError(err)
    }
}

impl<'r> Responder<'r> for ApiError {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
        match self.0 {
            Error::NotFound => Json(json!({
                "error": "Not found"
            })).respond_to(req),
            Error::Unauthorized => Json(json!({
                "error": "You are not authorized to access this resource"
            })).respond_to(req),
            _ => Json(json!({
                "error": "Server error"
            })).respond_to(req)
        }
    }
}

#[derive(FromForm)]
pub struct OAuthRequest {
    client_id: String,
    client_secret: String,
    password: String,
    username: String,
    scopes: String,
}

#[get("/oauth2?<query..>")]
pub fn oauth(query: Form<OAuthRequest>, conn: DbConn, searcher: Searcher) -> Result<Json<serde_json::Value>, ApiError> {
    let app = App::find_by_client_id(&*conn, &query.client_id)?;
    if app.client_secret == query.client_secret {
        if let Ok(user) = User::find_by_fqn(&Context::build(&*conn, &*searcher), &query.username) {
            if user.auth(&query.password) {
                let token = ApiToken::insert(&*conn, NewApiToken {
                    app_id: app.id,
                    user_id: user.id,
                    value: random_hex(),
                    scopes: query.scopes.clone(),
                })?;
                Ok(Json(json!({
                    "token": token.value
                })))
            } else {
                Ok(Json(json!({
                    "error": "Invalid credentials"
                })))
            }
        } else {
            // Making fake password verification to avoid different
            // response times that would make it possible to know
            // if a username is registered or not.
            User::get(&*conn, 1)?.auth(&query.password);
            Ok(Json(json!({
                "error": "Invalid credentials"
            })))
        }
    } else {
        Ok(Json(json!({
            "error": "Invalid client_secret"
        })))
    }
}

pub mod apps;
pub mod authorization;
pub mod posts;
