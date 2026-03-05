use plume_common::activity_pub::{
    inbox::FromId,
    request::Digest,
    sign::{verify_http_headers, Signable},
};
use plume_models::{
    db_conn::DbConn, headers::Headers, inbox::inbox, instance::Instance, users::User, Error, CONFIG,
};
use rocket::{data::*, http::Status, response::status, Request};
use rocket::serde::json::{Error as JsonError};
use serde::Deserialize;
use tracing::warn;

pub async fn handle_incoming(
    mut conn: DbConn,
    data: SignedJson<serde_json::Value>,
    headers: Headers<'_>,
) -> Result<String, status::BadRequest<&'static str>> {
    let act = data.1;
    let sig = data.0;

    let activity = act.clone();
    let actor_id = activity["actor"]
        .as_str()
        .or_else(|| activity["actor"]["id"].as_str())
        .ok_or(status::BadRequest("Missing actor id for activity"))?;

    let actor = User::from_id(&mut conn, actor_id, None, CONFIG.proxy()).await
        .expect("instance::shared_inbox: user error");
    if !verify_http_headers(&actor, &headers.0, &sig).is_secure() && !act.clone().verify(&actor) {
        // maybe we just know an old key?
        actor
            .refetch(&mut conn)
            .and_then(|_| User::get(&mut conn, actor.id))
            .and_then(|u| {
                if verify_http_headers(&u, &headers.0, &sig).is_secure() || act.clone().verify(&u) {
                    Ok(())
                } else {
                    Err(Error::Signature)
                }
            })
            .map_err(|_| {
                warn!(
                    "Rejected invalid activity supposedly from {}, with headers {:?}",
                    actor.username, headers.0
                );
                status::BadRequest("Invalid signature")
            })?;
    }

    if Instance::is_blocked(&mut conn, actor_id)
        .map_err(|_| status::BadRequest("Can't tell if instance is blocked"))?
    {
        return Ok(String::new());
    }

    Ok(match inbox(&mut conn, act).await {
        Ok(_) => String::new(),
        Err(e) => {
            warn!("Shared inbox error: {:?}", e);
            format!("Error: {:?}", e)
        }
    })
}

const JSON_LIMIT: ByteUnit = ByteUnit::Megabyte(10);

pub struct SignedJson<T>(pub Digest, pub T);

#[rocket::async_trait]
impl<'r,  T: Deserialize<'r>> FromData<'r> for SignedJson<T> {
    type Error = rocket::serde::json::Error<'r>;

    async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> rocket::data::Outcome<'r, Self> {
        let size_limit = req.limits().get("json").unwrap_or(JSON_LIMIT);
        match data.open(size_limit).into_string().await {
            Err(e) => Outcome::Error((Status::UnprocessableEntity, JsonError::Io(e))),
            Ok(js_data) => {
                let cached = rocket::request::local_cache!(req, js_data.into_inner());
                match serde_json::from_str(&cached) {
                    Ok(v) => Outcome::Success(SignedJson(Digest::from_body(&cached), v)),
                    Err(e) => Outcome::Error((Status::BadRequest, JsonError::Parse(&cached, e))),
                }
            },
        }
    }
}
