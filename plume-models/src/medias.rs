use activitypub::object::Image;
use diesel::{self, QueryDsl, ExpressionMethods, RunQueryDsl};
use guid_create::GUID;
use reqwest;
use serde_json;
use std::{fs, path::Path};

use plume_common::activity_pub::Id;

use {ap_url, Connection};
use instance::Instance;
use users::User;
use schema::medias;

#[derive(Clone, Identifiable, Queryable, Serialize)]
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

    pub fn to_json(&self, conn: &Connection) -> serde_json::Value {
        let mut json = serde_json::to_value(self).expect("Media::to_json: serialization error");
        let url = self.url(conn);
        let (preview, html, md) = match self.file_path.rsplitn(2, '.').next().expect("Media::to_json: extension error") {
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

    pub fn url(&self, conn: &Connection) -> String {
        if self.is_remote {
            self.remote_url.clone().unwrap_or(String::new())
        } else {
            ap_url(format!("{}/{}", Instance::get_local(conn).expect("Media::url: local instance not found error").public_domain, self.file_path))
        }
    }

    pub fn delete(&self, conn: &Connection) {
        fs::remove_file(self.file_path.as_str()).expect("Media::delete: file deletion error");
        diesel::delete(self).execute(conn).expect("Media::delete: database entry deletion error");
    }

    pub fn save_remote(conn: &Connection, url: String) -> Media {
        Media::insert(conn, NewMedia {
            file_path: String::new(),
            alt_text: String::new(),
            is_remote: true,
            remote_url: Some(url),
            sensitive: false,
            content_warning: None,
            owner_id: 1 // It will be owned by the admin during an instant, but set_owner will be called just after
        })
    }

    pub fn set_owner(&self, conn: &Connection, id: i32) {
        diesel::update(self)
            .set(medias::owner_id.eq(id))
            .execute(conn)
            .expect("Media::set_owner: owner update error");
    }

    // TODO: merge with save_remote?
    pub fn from_activity(conn: &Connection, image: Image) -> Option<Media> {
        let remote_url = image.object_props.url_string().ok()?;
        let ext = remote_url.rsplit('.').next().map(|ext| ext.to_owned()).unwrap_or("png".to_owned());
        let path = Path::new("static").join("media").join(format!("{}.{}", GUID::rand().to_string(), ext));

        let mut dest = fs::File::create(path.clone()).ok()?;
        reqwest::get(remote_url.as_str()).ok()?
            .copy_to(&mut dest).ok()?;

        Some(Media::insert(conn, NewMedia {
            file_path: path.to_str()?.to_string(),
            alt_text: image.object_props.content_string().ok()?,
            is_remote: true,
            remote_url: None,
            sensitive: image.object_props.summary_string().is_ok(),
            content_warning: image.object_props.summary_string().ok(),
            owner_id: User::from_url(conn, image.object_props.attributed_to_link_vec::<Id>().ok()?.into_iter().next()?.into())?.id
        }))
    }
}
