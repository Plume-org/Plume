use crate::{schema::apps, Error, Result};
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

#[derive(Clone, Queryable, serde::Serialize)]
pub struct App {
    pub id: i32,
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Option<String>,
    pub website: Option<String>,
    pub creation_date: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = apps)]
pub struct NewApp {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Option<String>,
    pub website: Option<String>,
}

impl App {
    get!(apps);
    insert!(apps, NewApp);
    find_by!(apps, find_by_client_id, client_id as &str);
}
