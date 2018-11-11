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

    pub fn list_all_medias(conn: &Connection) -> Vec<Media> {
        medias::table.load::<Media>(conn)
            .expect("Media::list_all_medias: loading error")
    }

    pub fn to_json(&self, conn: &Connection) -> serde_json::Value {
        let mut json = serde_json::to_value(self).expect("Media::to_json: serialization error");
        let url = self.url(conn);
        let (cat, preview, html, md) = match self.file_path.rsplitn(2, '.').next().expect("Media::to_json: extension error") {
            "png" | "jpg" | "jpeg" | "gif" | "svg" => (
                "image",
                format!("<img src=\"{}\" alt=\"{}\" title=\"{}\" class=\"preview\">", url, self.alt_text, self.alt_text),
                format!("<img src=\"{}\" alt=\"{}\" title=\"{}\">", url, self.alt_text, self.alt_text),
                format!("![{}]({})", self.alt_text, url),
            ),
            "mp3" | "wav" | "flac" => (
                "audio",
                format!("<audio src=\"{}\" title=\"{}\" class=\"preview\"></audio>", url, self.alt_text),
                format!("<audio src=\"{}\" title=\"{}\"></audio>", url, self.alt_text),
                format!("<audio src=\"{}\" title=\"{}\"></audio>", url, self.alt_text),
            ),
            "mp4" | "avi" | "webm" | "mov" => (
                "video",
                format!("<video src=\"{}\" title=\"{}\" class=\"preview\"></video>", url, self.alt_text),
                format!("<video src=\"{}\" title=\"{}\"></video>", url, self.alt_text),
                format!("<video src=\"{}\" title=\"{}\"></video>", url, self.alt_text),
            ),
            _ => ("unknown", String::new(), String::new(), String::new())
        };
        json["html_preview"] = json!(preview);
        json["html"] = json!(html);
        json["url"] = json!(url);
        json["md"] = json!(md);
        json["category"] = json!(cat);
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
        if !self.is_remote {
            fs::remove_file(self.file_path.as_str()).expect("Media::delete: file deletion error");
        }
        diesel::delete(self).execute(conn).expect("Media::delete: database entry deletion error");
    }

    pub fn save_remote(conn: &Connection, url: String, user: &User) -> Media {
        Media::insert(conn, NewMedia {
            file_path: String::new(),
            alt_text: String::new(),
            is_remote: true,
            remote_url: Some(url),
            sensitive: false,
            content_warning: None,
            owner_id: user.id
        })
    }

    pub fn set_owner(&self, conn: &Connection, user: &User) {
        diesel::update(self)
            .set(medias::owner_id.eq(user.id))
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



#[cfg(test)]
pub(crate) mod tests {
    use diesel::Connection;
    use Connection as Conn;
    use tests::db;
    use super::*;
    use users::tests as usersTests;
    use std::fs;
    use std::path::Path;
    use std::env::{current_dir, set_current_dir};

    pub(crate) fn fill_database(conn: &Conn) -> Vec<Media>{
        let mut wd = current_dir().unwrap().to_path_buf();
        while wd.pop() {
            if wd.join(".git").exists() {
                set_current_dir(wd).unwrap();
                break;
            }
        }

        let users = usersTests::fill_database(conn);
        let user_one = users[0].id;
        let user_two = users[1].id;
        let f1 = "static/media/1.png".to_owned();
        let f2 = "static/media/2.mp3".to_owned();
        fs::write(f1.clone(), []).unwrap();
        fs::write(f2.clone(), []).unwrap();
        vec![NewMedia {
                file_path: f1,
                alt_text: "some alt".to_owned(),
                is_remote: false,
                remote_url: None,
                sensitive: false,
                content_warning: None,
                owner_id: user_one,
            },
            NewMedia {
                file_path: f2,
                alt_text: "alt message".to_owned(),
                is_remote: false,
                remote_url: None,
                sensitive: true,
                content_warning: Some("Content warning".to_owned()),
                owner_id: user_one,
            },
            NewMedia {
                file_path: "".to_owned(),
                alt_text: "another alt".to_owned(),
                is_remote: true,
                remote_url: Some("https://example.com/".to_owned()),
                sensitive: false,
                content_warning: None,
                owner_id: user_two,
            },
        ].into_iter().map(|nm| Media::insert(conn, nm))
            .collect()
    }

    pub(crate) fn clean(conn: &Conn) {//used to remove files generated by tests
        for media in Media::list_all_medias(conn) {
            media.delete(conn);
        }
    }


    //set_owner

    #[test]
    fn delete() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let user = usersTests::fill_database(conn)[0].id;
            fill_database(conn);

            let path = "static/media/test_deletion".to_owned();
            fs::write(path.clone(), []).unwrap();

            let media = Media::insert(conn, NewMedia {
                file_path: path.clone(),
                alt_text: "alt message".to_owned(),
                is_remote: false,
                remote_url: None,
                sensitive: false,
                content_warning: None,
                owner_id: user,
            });

            assert!(Path::new(&path).exists());
            media.delete(conn);
            assert!(!Path::new(&path).exists());

            clean(conn);

            Ok(())
        });
    }

    #[test]

    fn set_owner() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let users = usersTests::fill_database(conn);
            let u1 = &users[0];
            let u2 = &users[1];
            fill_database(conn);

            let path = "static/media/test_set_owner".to_owned();
            fs::write(path.clone(), []).unwrap();

            let media = Media::insert(conn, NewMedia {
                file_path: path.clone(),
                alt_text: "alt message".to_owned(),
                is_remote: false,
                remote_url: None,
                sensitive: false,
                content_warning: None,
                owner_id: u1.id,
            });

            assert!(Media::for_user(conn, u1.id).iter().any(|m| m.id==media.id));
            assert!(!Media::for_user(conn, u2.id).iter().any(|m| m.id==media.id));
            media.set_owner(conn, u2);
            assert!(!Media::for_user(conn, u1.id).iter().any(|m| m.id==media.id));
            assert!(Media::for_user(conn, u2.id).iter().any(|m| m.id==media.id));

            clean(conn);

            Ok(())
        });
     }
}
