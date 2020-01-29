use crate::{
    ap_url, instance::Instance, safe_string::SafeString, schema::medias, users::User, Connection,
    Error, PlumeRocket, Result,
};
use activitypub::object::Image;
use askama_escape::escape;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use guid_create::GUID;
use plume_common::{
    activity_pub::{inbox::FromId, Id},
    utils::MediaProcessor,
};
use reqwest;
use std::{fs, path::Path};
use tokio::prelude::*;

#[derive(Clone, Identifiable, Queryable)]
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

impl MediaCategory {
    pub fn to_string(&self) -> &str {
        match *self {
            MediaCategory::Image => "image",
            MediaCategory::Audio => "audio",
            MediaCategory::Video => "video",
            MediaCategory::Unknown => "unknown",
        }
    }
}

impl Media {
    insert!(medias, NewMedia);
    get!(medias);

    pub fn for_user(conn: &Connection, owner: i32) -> Result<Vec<Media>> {
        medias::table
            .filter(medias::owner_id.eq(owner))
            .order(medias::id.desc())
            .load::<Self>(conn)
            .map_err(Error::from)
    }

    pub fn list_all_medias(conn: &Connection) -> Result<Vec<Media>> {
        medias::table.load::<Media>(conn).map_err(Error::from)
    }

    pub fn page_for_user(
        conn: &Connection,
        user: &User,
        (min, max): (i32, i32),
    ) -> Result<Vec<Media>> {
        medias::table
            .filter(medias::owner_id.eq(user.id))
            .order(medias::id.desc())
            .offset(i64::from(min))
            .limit(i64::from(max - min))
            .load::<Media>(conn)
            .map_err(Error::from)
    }

    pub fn count_for_user(conn: &Connection, user: &User) -> Result<i64> {
        medias::table
            .filter(medias::owner_id.eq(user.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn category(&self) -> MediaCategory {
        match &*self
            .file_path
            .rsplitn(2, '.')
            .next()
            .expect("Media::category: extension error")
            .to_lowercase()
        {
            "png" | "jpg" | "jpeg" | "gif" | "svg" => MediaCategory::Image,
            "mp3" | "wav" | "flac" => MediaCategory::Audio,
            "mp4" | "avi" | "webm" | "mov" => MediaCategory::Video,
            _ => MediaCategory::Unknown,
        }
    }

    pub fn html(&self) -> Result<SafeString> {
        let url = self.url()?;
        Ok(match self.category() {
            MediaCategory::Image => SafeString::trusted(&format!(
                r#"<img src="{}" alt="{}" title="{}">"#,
                url,
                escape(&self.alt_text),
                escape(&self.alt_text)
            )),
            MediaCategory::Audio => SafeString::trusted(&format!(
                r#"<div class="media-preview audio"></div><audio src="{}" title="{}" controls></audio>"#,
                url,
                escape(&self.alt_text)
            )),
            MediaCategory::Video => SafeString::trusted(&format!(
                r#"<video src="{}" title="{}" controls></video>"#,
                url,
                escape(&self.alt_text)
            )),
            MediaCategory::Unknown => SafeString::trusted(&format!(
                r#"<a href="{}" class="media-preview unknown"></a>"#,
                url,
            )),
        })
    }

    pub fn markdown(&self) -> Result<SafeString> {
        Ok(match self.category() {
            MediaCategory::Image => {
                SafeString::new(&format!("![{}]({})", escape(&self.alt_text), self.id))
            }
            MediaCategory::Audio | MediaCategory::Video => self.html()?,
            MediaCategory::Unknown => SafeString::new(""),
        })
    }

    pub fn url(&self) -> Result<String> {
        if self.is_remote {
            Ok(self.remote_url.clone().unwrap_or_default())
        } else {
            let p = Path::new(&self.file_path);
            let filename: String = p.file_name().unwrap().to_str().unwrap().to_owned();
            Ok(ap_url(&format!(
                "{}/static/media/{}",
                Instance::get_local()?.public_domain,
                &filename
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
    pub async fn from_activity(c: &PlumeRocket, image: &Image) -> Result<Media> {
        let conn = &*c.conn;
        let remote_url = image.object_props.url_string().ok()?;
        let ext = remote_url
            .rsplit('.')
            .next()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| String::from("png"));
        let path = Path::new(&super::CONFIG.media_directory).join(format!(
            "{}.{}",
            GUID::rand().to_string(),
            ext
        ));

        let mut dest = tokio::fs::File::create(path.clone()).await?;
        let contents = reqwest::get(remote_url.as_str())
            .await?
            .bytes()
            .await?;
        dest.write_all(&contents).await?;

        Media::insert(
            conn,
            NewMedia {
                file_path: path.to_str()?.to_string(),
                alt_text: image.object_props.content_string().ok()?,
                is_remote: false,
                remote_url: None,
                sensitive: image.object_props.summary_string().is_ok(),
                content_warning: image.object_props.summary_string().ok(),
                owner_id: User::from_id(
                    c,
                    image
                        .object_props
                        .attributed_to_link_vec::<Id>()
                        .ok()?
                        .into_iter()
                        .next()?
                        .as_ref(),
                    None,
                )
                .map_err(|(_, e)| e)?
                .id,
            },
        )
    }

    pub fn get_media_processor<'a>(conn: &'a Connection, user: Vec<&User>) -> MediaProcessor<'a> {
        let uid = user.iter().map(|u| u.id).collect::<Vec<_>>();
        Box::new(move |id| {
            let media = Media::get(conn, id).ok()?;
            // if owner is user or check is disabled
            if uid.contains(&media.owner_id) || uid.is_empty() {
                Some((media.url().ok()?, media.content_warning))
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{tests::db, users::tests as usersTests, Connection as Conn};
    use diesel::Connection;
    use std::env::{current_dir, set_current_dir};
    use std::fs;
    use std::path::Path;

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
        (
            users,
            vec![
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
            ]
            .into_iter()
            .map(|nm| Media::insert(conn, nm).unwrap())
            .collect(),
        )
    }

    pub(crate) fn clean(conn: &Conn) {
        //used to remove files generated by tests
        for media in Media::list_all_medias(conn).unwrap() {
            media.delete(conn).unwrap();
        }
    }

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
            )
            .unwrap();

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
            )
            .unwrap();

            assert!(Media::for_user(conn, u1.id)
                .unwrap()
                .iter()
                .any(|m| m.id == media.id));
            assert!(!Media::for_user(conn, u2.id)
                .unwrap()
                .iter()
                .any(|m| m.id == media.id));
            media.set_owner(conn, u2).unwrap();
            assert!(!Media::for_user(conn, u1.id)
                .unwrap()
                .iter()
                .any(|m| m.id == media.id));
            assert!(Media::for_user(conn, u2.id)
                .unwrap()
                .iter()
                .any(|m| m.id == media.id));

            clean(conn);
            Ok(())
        });
    }
}
