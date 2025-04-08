use plume_common::activity_pub::{
    inbox::FromId,
    request::Digest,
    sign::{verify_http_headers, Signable},
};
use plume_models::{
    db_conn::DbConn, headers::Headers, inbox::inbox, instance::Instance, users::User, Error, CONFIG,
};
use rocket::{data::*, http::Status, response::status, Outcome::*, Request};
use rocket_contrib::json::*;
use serde::Deserialize;
use std::io::Read;
use tracing::warn;

pub fn handle_incoming(
    conn: DbConn,
    data: SignedJson<serde_json::Value>,
    headers: Headers<'_>,
) -> Result<String, status::BadRequest<&'static str>> {
    let act = data.1.into_inner();
    let sig = data.0;

    let activity = act.clone();
    let actor_id = activity["actor"]
        .as_str()
        .or_else(|| activity["actor"]["id"].as_str())
        .ok_or(status::BadRequest(Some("Missing actor id for activity")))?;

    let actor = match User::from_id(&conn, actor_id, None, CONFIG.proxy()) {
        Ok(actor) => actor,
        // ignore activity from deleted actor
        Err((Some(json), _)) if json.get("error").map(|v| v == "Gone").unwrap_or(false) => return Ok(String::new()),
        Err(e) => {
            warn!("failed to resolve user from id: {e:?}");
            return Err(status::BadRequest(Some("unresolvable actor")));
        }
    };

    if !verify_http_headers(&actor, &headers.0, &sig).is_secure() && !act.clone().verify(&actor) {
        // maybe we just know an old key?
        actor
            .refetch(&conn)
            .and_then(|_| User::get(&conn, actor.id))
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
                status::BadRequest(Some("Invalid signature"))
            })?;
    }

    if Instance::is_blocked(&conn, actor_id)
        .map_err(|_| status::BadRequest(Some("Can't tell if instance is blocked")))?
    {
        return Ok(String::new());
    }

    Ok(match inbox(&conn, act) {
        Ok(_) => String::new(),
        Err(e) => {
            warn!("Shared inbox error: {:?}", e);
            format!("Error: {:?}", e)
        }
    })
}

const JSON_LIMIT: u64 = 1 << 20;

pub struct SignedJson<T>(pub Digest, pub Json<T>);

impl<'a, T: Deserialize<'a>> FromData<'a> for SignedJson<T> {
    type Error = JsonError<'a>;
    type Owned = String;
    type Borrowed = str;

    fn transform(
        r: &Request<'_>,
        d: Data,
    ) -> Transform<rocket::data::Outcome<Self::Owned, Self::Error>> {
        let size_limit = r.limits().get("json").unwrap_or(JSON_LIMIT);
        let mut s = String::with_capacity(512);
        match d.open().take(size_limit).read_to_string(&mut s) {
            Ok(_) => Transform::Borrowed(Success(s)),
            Err(e) => Transform::Borrowed(Failure((Status::BadRequest, JsonError::Io(e)))),
        }
    }

    fn from_data(
        _: &Request<'_>,
        o: Transformed<'a, Self>,
    ) -> rocket::data::Outcome<Self, Self::Error> {
        let string = o.borrowed()?;
        match serde_json::from_str(string) {
            Ok(v) => Success(SignedJson(Digest::from_body(string), Json(v))),
            Err(e) => {
                if e.is_data() {
                    Failure((Status::UnprocessableEntity, JsonError::Parse(string, e)))
                } else {
                    Failure((Status::BadRequest, JsonError::Parse(string, e)))
                }
            }
        }
    }
}
