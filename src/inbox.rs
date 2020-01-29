use plume_common::activity_pub::{
    inbox::FromId,
    request::Digest,
    sign::{verify_http_headers, Signable},
};
use plume_models::{
    headers::Headers, inbox::inbox, instance::Instance, users::User, Error, PlumeRocket,
};
use rocket::{data::*, http::Status, response::status, Outcome::*, Request};
use rocket_contrib::json::*;
use serde::Deserialize;
use std::io::Read;

pub fn handle_incoming(
    rockets: PlumeRocket,
    data: SignedJson<serde_json::Value>,
    headers: Headers<'_>,
) -> Result<String, status::BadRequest<&'static str>> {
    let conn = &*rockets.conn;
    let act = data.1.into_inner();
    let sig = data.0;

    let activity = act.clone();
    let actor_id = activity["actor"]
        .as_str()
        .or_else(|| activity["actor"]["id"].as_str())
        .ok_or(status::BadRequest(Some("Missing actor id for activity")))?;

    let actor =
        User::from_id(&rockets, actor_id, None).expect("instance::shared_inbox: user error");
    if !verify_http_headers(&actor, &headers.0, &sig).is_secure() && !act.clone().verify(&actor) {
        // maybe we just know an old key?
        actor
            .refetch(conn)
            .and_then(|_| User::get(conn, actor.id))
            .and_then(|u| {
                if verify_http_headers(&u, &headers.0, &sig).is_secure() || act.clone().verify(&u) {
                    Ok(())
                } else {
                    Err(Error::Signature)
                }
            })
            .map_err(|_| {
                println!(
                    "Rejected invalid activity supposedly from {}, with headers {:?}",
                    actor.username, headers.0
                );
                status::BadRequest(Some("Invalid signature"))
            })?;
    }

    if Instance::is_blocked(conn, actor_id)
        .map_err(|_| status::BadRequest(Some("Can't tell if instance is blocked")))?
    {
        return Ok(String::new());
    }

    Ok(match inbox(&rockets, act) {
        Ok(_) => String::new(),
        Err(e) => {
            println!("Shared inbox error: {:?}", e);
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

    fn transform<'r>(
        r: &'r Request,
        d: Data
    ) -> TransformFuture<'r, Self::Owned, Self::Error> {
        Box::pin(async move {
            let size_limit = r.limits().get("json").unwrap_or(JSON_LIMIT);
            let mut s = String::with_capacity(512);
            let outcome = match d.open().take(size_limit).read_to_string(&mut s) {
                Ok(_) => Success(s),
                Err(e) => Failure((Status::BadRequest, JsonError::Io(e))),
            };
            Transform::Borrowed(outcome)
        })
    }

    fn from_data(
        _: &Request<'_>,
        o: Transformed<'a, Self>,
    ) -> FromDataFuture<'a, Self, Self::Error> {
        Box::pin(async move {
            let string = try_outcome!(o.borrowed());
            match serde_json::from_str(&string) {
                Ok(v) => Success(SignedJson(Digest::from_body(&string), Json(v))),
                Err(e) if e.is_data() => return Failure((Status::UnprocessableEntity, JsonError::Parse(string, e))),
                Err(e) => Failure((Status::BadRequest, JsonError::Parse(string, e))),
            }
        })
    }
}
