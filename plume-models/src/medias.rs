use diesel::{self, PgConnection, QueryDsl, ExpressionMethods, RunQueryDsl};
use serde_json;
use std::fs;

use ap_url;
use instance::Instance;
use schema::medias;

#[derive(Identifiable, Queryable, Serialize)]
pub struct Media {
    pub id: i32,
    pub file_path: String,
    pub alt_text: String,
    pub is_remote: bool,
    pub remote_url: Option<String>,
    pub sensitive: bool,
    pub content_warning: Option<String>,
    pub owner_id: i32
}

#[derive(Insertable)]
#[table_name = "medias"]
pub struct NewMedia {
    pub file_path: String,
    pub alt_text: String,
    pub is_remote: bool,
    pub remote_url: Option<String>,
    pub sensitive: bool,
    pub content_warning: Option<String>,
    pub owner_id: i32
}

impl Media {
    insert!(medias, NewMedia);
    get!(medias);
    list_by!(medias, for_user, owner_id as i32);

    pub fn to_json(&self, conn: &PgConnection) -> serde_json::Value {
        let mut json = serde_json::to_value(self).unwrap();
        let url = self.url(conn);
        let (preview, html, md) = match self.file_path.rsplitn(2, '.').next().unwrap() {
            "png" | "jpg" | "jpeg" | "gif" | "svg" => (
                format!("<img src=\"{}\" alt=\"{}\" title=\"{}\" class=\"preview\">", url, self.alt_text, self.alt_text),
                format!("<img src=\"{}\" alt=\"{}\" title=\"{}\">", url, self.alt_text, self.alt_text),
                format!("![{}]({})", self.alt_text, url),
            ),
            "mp3" | "wav" | "flac" => (
                format!("<audio src=\"{}\" title=\"{}\" class=\"preview\"></audio>", url, self.alt_text),
                format!("<audio src=\"{}\" title=\"{}\"></audio>", url, self.alt_text),
                format!("<audio src=\"{}\" title=\"{}\"></audio>", url, self.alt_text),
            ),
            "mp4" | "avi" | "webm" | "mov" => (
                format!("<video src=\"{}\" title=\"{}\" class=\"preview\"></video>", url, self.alt_text),
                format!("<video src=\"{}\" title=\"{}\"></video>", url, self.alt_text),
                format!("<video src=\"{}\" title=\"{}\"></video>", url, self.alt_text),
            ),
            _ => (String::new(), String::new(), String::new())
        };
        json["html_preview"] = json!(preview);
        json["html"] = json!(html);
        json["url"] = json!(url);
        json["md"] = json!(md);
        json
    }

    pub fn url(&self, conn: &PgConnection) -> String {
        ap_url(format!("{}/static/{}", Instance::get_local(conn).unwrap().public_domain, self.file_path))
    }

    pub fn delete(&self, conn: &PgConnection) {
        fs::remove_file(self.file_path.as_str()).expect("Couldn't delete media from disk");
        diesel::delete(self).execute(conn).expect("Couldn't remove media from DB");
    }
}
