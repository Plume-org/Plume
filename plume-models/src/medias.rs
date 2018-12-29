use activitypub::object::Image;
use askama_escape::escape;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use guid_create::GUID;
use reqwest;
use std::{fs, path::Path};

use plume_common::activity_pub::Id;

use instance::Instance;
use safe_string::SafeString;
use schema::medias;
use users::User;
use {ap_url, Connection, Error, Result};

#[derive(Clone, Identifiable, Queryable, Serialize)]
pub struct Media {
    pub id: i32,
    pub file_path: String,
    pub alt_text: String,
    pub is_remote: bool,
    pub remote_url: Option<String>,
    pub sensitive: bool,
    pub content_warning: Option<String>,
    pub owner_id: i32,
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
    pub owner_id: i32,
}

#[derive(PartialEq)]
pub enum MediaCategory {
    Image,
    Audio,
    Video,
    Unknown,
}

impl Media {
    insert!(medias, NewMedia);
    get!(medias);
    list_by!(medias, for_user, owner_id as i32);

    pub fn list_all_medias(conn: &Connection) -> Result<Vec<Media>> {
        medias::table
            .load::<Media>(conn)
            .map_err(Error::from)
    }

    pub fn category(&self) -> MediaCategory {
        match self
            .file_path
            .rsplitn(2, '.')
            .next()
            .expect("Media::category: extension error")
        {
            "png" | "jpg" | "jpeg" | "gif" | "svg" => MediaCategory::Image,
            "mp3" | "wav" | "flac" => MediaCategory::Audio,
            "mp4" | "avi" | "webm" | "mov" => MediaCategory::Video,
            _ => MediaCategory::Unknown,
        }
    }

    pub fn preview_html(&self, conn: &Connection) -> Result<SafeString> {
        let url = self.url(conn)?;
        Ok(match self.category() {
            MediaCategory::Image => SafeString::new(&format!(
                r#"<img src="{}" alt="{}" title="{}" class=\"preview\">"#,
                url, escape(&self.alt_text), escape(&self.alt_text)
            )),
            MediaCategory::Audio => SafeString::new(&format!(
                r#"<audio src="{}" title="{}" class="preview"></audio>"#,
                url, escape(&self.alt_text)
            )),
            MediaCategory::Video => SafeString::new(&format!(
                r#"<video src="{}" title="{}" class="preview"></video>"#,
                url, escape(&self.alt_text)
            )),
            MediaCategory::Unknown => SafeString::new(""),
        })
    }

    pub fn html(&self, conn: &Connection) -> Result<SafeString> {
        let url = self.url(conn)?;
        Ok(match self.category() {
            MediaCategory::Image => SafeString::new(&format!(
                r#"<img src="{}" alt="{}" title="{}">"#,
                url, escape(&self.alt_text), escape(&self.alt_text)
            )),
            MediaCategory::Audio => SafeString::new(&format!(
                r#"<audio src="{}" title="{}"></audio>"#,
                url, escape(&self.alt_text)
            )),
            MediaCategory::Video => SafeString::new(&format!(
                r#"<video src="{}" title="{}"></video>"#,
                url, escape(&self.alt_text)
            )),
            MediaCategory::Unknown => SafeString::new(""),
        })
    }

    pub fn markdown(&self, conn: &Connection) -> Result<SafeString> {
        let url = self.url(conn)?;
        Ok(match self.category() {
            MediaCategory::Image => SafeString::new(&format!("![{}]({})", escape(&self.alt_text), url)),
            MediaCategory::Audio | MediaCategory::Video => self.html(conn)?,
            MediaCategory::Unknown => SafeString::new(""),
        })
    }

    pub fn url(&self, conn: &Connection) -> Result<String> {
        if self.is_remote {
            Ok(self.remote_url.clone().unwrap_or_default())
        } else {
            Ok(ap_url(&format!(
                "{}/{}",
                Instance::get_local(conn)?.public_domain,
                self.file_path
            )))
        }
    }

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        if !self.is_remote {
            fs::remove_file(self.file_path.as_str())?;
        }
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn save_remote(conn: &Connection, url: String, user: &User) -> Result<Media> {
        if url.contains(&['<', '>', '"'][..]) {
            Err(Error::Url)
        } else {
            Media::insert(
                conn,
                NewMedia {
                    file_path: String::new(),
                    alt_text: String::new(),
                    is_remote: true,
                    remote_url: Some(url),
                    sensitive: false,
                    content_warning: None,
                    owner_id: user.id,
                },
            )
        }
    }

    pub fn set_owner(&self, conn: &Connection, user: &User) -> Result<()> {
        diesel::update(self)
            .set(medias::owner_id.eq(user.id))
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    // TODO: merge with save_remote?
    pub fn from_activity(conn: &Connection, image: &Image) -> Result<Media> {
        let remote_url = image.object_props.url_string().ok()?;
        let ext = remote_url
            .rsplit('.')
            .next()
            .map(|ext| ext.to_owned())
            .unwrap_or_else(|| String::from("png"));
        let path =
            Path::new("static")
                .join("media")
                .join(format!("{}.{}", GUID::rand().to_string(), ext));

        let mut dest = fs::File::create(path.clone()).ok()?;
        reqwest::get(remote_url.as_str())
            .ok()?
            .copy_to(&mut dest)
            .ok()?;

        Media::insert(
            conn,
            NewMedia {
                file_path: path.to_str()?.to_string(),
                alt_text: image.object_props.content_string().ok()?,
                is_remote: false,
                remote_url: None,
                sensitive: image.object_props.summary_string().is_ok(),
                content_warning: image.object_props.summary_string().ok(),
                owner_id: User::from_url(
                    conn,
                    image
                        .object_props
                        .attributed_to_link_vec::<Id>()
                        .ok()?
                        .into_iter()
                        .next()?
                        .as_ref(),
                )?.id,
            },
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use diesel::Connection;
    use std::env::{current_dir, set_current_dir};
    use std::fs;
    use std::path::Path;
    use tests::db;
    use users::tests as usersTests;
    use Connection as Conn;

    pub(crate) fn fill_database(conn: &Conn) -> (Vec<User>, Vec<Media>) {
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
        (users, vec![
            NewMedia {
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
        ].into_iter()
            .map(|nm| Media::insert(conn, nm).unwrap())
            .collect())
    }

    pub(crate) fn clean(conn: &Conn) {
        //used to remove files generated by tests
        for media in Media::list_all_medias(conn).unwrap() {
            media.delete(conn).unwrap();
        }
    }

    //set_owner

    #[test]
    fn delete() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let user = fill_database(conn).0[0].id;

            let path = "static/media/test_deletion".to_owned();
            fs::write(path.clone(), []).unwrap();

            let media = Media::insert(
                conn,
                NewMedia {
                    file_path: path.clone(),
                    alt_text: "alt message".to_owned(),
                    is_remote: false,
                    remote_url: None,
                    sensitive: false,
                    content_warning: None,
                    owner_id: user,
                },
            ).unwrap();

            assert!(Path::new(&path).exists());
            media.delete(conn).unwrap();
            assert!(!Path::new(&path).exists());

            clean(conn);

            Ok(())
        });
    }

    #[test]

    fn set_owner() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, _) = fill_database(conn);
            let u1 = &users[0];
            let u2 = &users[1];

            let path = "static/media/test_set_owner".to_owned();
            fs::write(path.clone(), []).unwrap();

            let media = Media::insert(
                conn,
                NewMedia {
                    file_path: path.clone(),
                    alt_text: "alt message".to_owned(),
                    is_remote: false,
                    remote_url: None,
                    sensitive: false,
                    content_warning: None,
                    owner_id: u1.id,
                },
            ).unwrap();

            assert!(
                Media::for_user(conn, u1.id).unwrap()
                    .iter()
                    .any(|m| m.id == media.id)
            );
            assert!(
                !Media::for_user(conn, u2.id).unwrap()
                    .iter()
                    .any(|m| m.id == media.id)
            );
            media.set_owner(conn, u2).unwrap();
            assert!(
                !Media::for_user(conn, u1.id).unwrap()
                    .iter()
                    .any(|m| m.id == media.id)
            );
            assert!(
                Media::for_user(conn, u2.id).unwrap()
                    .iter()
                    .any(|m| m.id == media.id)
            );

            clean(conn);

            Ok(())
        });
    }
}
