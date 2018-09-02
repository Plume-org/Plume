use diesel::{self, PgConnection, QueryDsl, ExpressionMethods, RunQueryDsl};
use schema::medias;

#[derive(Queryable)]
pub struct Media {
    pub id: i32,
    pub file_path: String,
    pub alt_text: String,
    pub is_remote: bool,
    pub remote_url: Option<String>,
    pub sensitive: bool,
    pub content_warning: Option<String>
}

#[derive(Insertable)]
#[table_name = "medias"]
pub struct NewMedia {
    pub file_path: String,
    pub alt_text: String,
    pub is_remote: bool,
    pub remote_url: Option<String>,
    pub sensitive: bool,
    pub content_warning: Option<String>
}

impl Media {
    insert!(medias, NewMedia);
    get!(medias);
}
