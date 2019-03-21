use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use rocket::{
    http::Status,
    request::{self, FromRequest, Request},
    Outcome,
};

use db_conn::DbConn;
use schema::api_tokens;
use {Error, Result};

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
    find_by!(api_tokens, find_by_value, value as &str);

    pub fn can(&self, what: &'static str, scope: &'static str) -> bool {
        let full_scope = what.to_owned() + ":" + scope;
        for s in self.scopes.split('+') {
            if s == what || s == full_scope {
                return true;
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

#[derive(Debug)]
pub enum TokenError {
    /// The Authorization header was not present
    NoHeader,

    /// The type of the token was not specified ("Basic" or "Bearer" for instance)
    NoType,

    /// No value was provided
    NoValue,

    /// Error while connecting to the database to retrieve all the token metadata
    DbError,
}

impl<'a, 'r> FromRequest<'a, 'r> for ApiToken {
    type Error = TokenError;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<ApiToken, TokenError> {
        let headers: Vec<_> = request.headers().get("Authorization").collect();
        if headers.len() != 1 {
            return Outcome::Failure((Status::BadRequest, TokenError::NoHeader));
        }

        let mut parsed_header = headers[0].split(' ');
        let auth_type = parsed_header.next().map_or_else(
            || Outcome::Failure((Status::BadRequest, TokenError::NoType)),
            Outcome::Success,
        )?;
        let val = parsed_header.next().map_or_else(
            || Outcome::Failure((Status::BadRequest, TokenError::NoValue)),
            Outcome::Success,
        )?;

        if auth_type == "Bearer" {
            let conn = request
                .guard::<DbConn>()
                .map_failure(|_| (Status::InternalServerError, TokenError::DbError))?;
            if let Ok(token) = ApiToken::find_by_value(&*conn, val) {
                return Outcome::Success(token);
            }
        }

        Outcome::Forward(())
    }
}
