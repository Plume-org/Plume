use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use rocket::{
    Outcome,
    http::Status,
    request::{self, FromRequest, Request}
};

use db_conn::DbConn;
use schema::api_tokens;

#[derive(Clone, Queryable)]
pub struct ApiToken {
    pub id: i32,
    pub creation_date: NaiveDateTime,
    pub value: String,

    /// Scopes, separated by +
    /// Global scopes are read and write
    /// and both can be limited to an endpoint by affixing them with :ENDPOINT
    ///
    /// Examples :
    ///
    /// read
    /// read+write
    /// read:posts
    /// read:posts+write:posts
    pub scopes: String,
    pub app_id: i32,
    pub user_id: i32,
}

#[derive(Insertable)]
#[table_name = "api_tokens"]
pub struct NewApiToken {
    pub value: String,
    pub scopes: String,
    pub app_id: i32,
    pub user_id: i32,
}

impl ApiToken {
    get!(api_tokens);
    insert!(api_tokens, NewApiToken);
    find_by!(api_tokens, find_by_value, value as String);

    pub fn can(&self, what: &'static str, scope: &'static str) -> bool {
        let full_scope = what.to_owned() + ":" + scope;
        for s in self.scopes.split('+') {
            if s == what || s == full_scope {
                return true
            }
        }
        false
    }

    pub fn can_read(&self, scope: &'static str) -> bool {
        self.can("read", scope)
    }

    pub fn can_write(&self, scope: &'static str) -> bool {
        self.can("write", scope)
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for ApiToken {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<ApiToken, ()> {
        let headers: Vec<_> = request.headers().get("Authorization").collect();
        if headers.len() != 1 {
            return Outcome::Failure((Status::BadRequest, ()));
        }

        let mut parsed_header = headers[0].split(' ');
        let auth_type = parsed_header.next().expect("Expect a token type");
        let val = parsed_header.next().expect("Expect a token value");

        if auth_type == "Bearer" {
            let conn = request.guard::<DbConn>().expect("Couldn't connect to DB");
            if let Some(token) = ApiToken::find_by_value(&*conn, val.to_string()) {
                return Outcome::Success(token);
            }
        }

        return Outcome::Forward(());
    }
}
