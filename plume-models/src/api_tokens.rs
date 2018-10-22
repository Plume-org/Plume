use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

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
}
