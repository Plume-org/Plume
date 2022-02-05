use crate::{
    ap_url, db_conn::DbConn, instance::Instance, safe_string::SafeString, schema::medias,
    users::User, Connection, Error, Result, CONFIG,
};
use activitypub::object::Image;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use guid_create::GUID;
use plume_common::{
    activity_pub::{inbox::FromId, request, Id},
    utils::{escape, MediaProcessor},
};
use std::{
    fs::{self, DirBuilder},
    path::{self, Path, PathBuf},
};
use tracing::warn;
use url::Url;

const REMOTE_MEDIA_DIRECTORY: &str = "remote";

#[derive(Clone, Identifiable, Queryable, AsChangeset)]
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
    find_by!(medias, find_by_file_path, file_path as &str);

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
            .rsplit_once('.')
            .map(|x| x.1)
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
            let file_path = self.file_path.replace(path::MAIN_SEPARATOR, "/").replacen(
                &CONFIG.media_directory,
                "static/media",
                1,
            ); // "static/media" from plume::routs::plume_media_files()
            Ok(ap_url(&format!(
                "{}/{}",
                Instance::get_local()?.public_domain,
                &file_path
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
    pub fn from_activity(conn: &DbConn, image: &Image) -> Result<Media> {
        let remote_url = image
            .object_props
            .url_string()
            .or(Err(Error::MissingApProperty))?;
        let path = determine_mirror_file_path(&remote_url);
        let parent = path.parent().ok_or(Error::InvalidValue)?;
        if !parent.is_dir() {
            DirBuilder::new().recursive(true).create(parent)?;
        }

        let mut dest = fs::File::create(path.clone())?;
        // TODO: conditional GET
        request::get(
            remote_url.as_str(),
            User::get_sender(),
            CONFIG.proxy().cloned(),
        )?
        .copy_to(&mut dest)?;

        Media::find_by_file_path(conn, path.to_str().ok_or(Error::InvalidValue)?)
            .and_then(|mut media| {
                let mut updated = false;

                let alt_text = image
                    .object_props
                    .content_string()
                    .or(Err(Error::NotFound))?;
                let sensitive = image.object_props.summary_string().is_ok();
                let content_warning = image.object_props.summary_string().ok();
                if media.alt_text != alt_text {
                    media.alt_text = alt_text;
                    updated = true;
                }
                if media.is_remote {
                    media.is_remote = false;
                    updated = true;
                }
                if media.remote_url.is_some() {
                    media.remote_url = None;
                    updated = true;
                }
                if media.sensitive != sensitive {
                    media.sensitive = sensitive;
                    updated = true;
                }
                if media.content_warning != content_warning {
                    media.content_warning = content_warning;
                    updated = true;
                }
                if updated {
                    diesel::update(&media).set(&media).execute(&**conn)?;
                }
                Ok(media)
            })
            .or_else(|_| {
                Media::insert(
                    conn,
                    NewMedia {
                        file_path: path.to_str().ok_or(Error::InvalidValue)?.to_string(),
                        alt_text: image
                            .object_props
                            .content_string()
                            .or(Err(Error::NotFound))?,
                        is_remote: false,
                        remote_url: None,
                        sensitive: image.object_props.summary_string().is_ok(),
                        content_warning: image.object_props.summary_string().ok(),
                        owner_id: User::from_id(
                            conn,
                            image
                                .object_props
                                .attributed_to_link_vec::<Id>()
                                .or(Err(Error::NotFound))?
                                .into_iter()
                                .next()
                                .ok_or(Error::NotFound)?
                                .as_ref(),
                            None,
                            CONFIG.proxy(),
                        )
                        .map_err(|(_, e)| e)?
                        .id,
                    },
                )
            })
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

fn determine_mirror_file_path(url: &str) -> PathBuf {
    let mut file_path = Path::new(&super::CONFIG.media_directory).join(REMOTE_MEDIA_DIRECTORY);
    Url::parse(url)
        .map(|url| {
            if !url.has_host() {
                return;
            }
            file_path.push(url.host_str().unwrap());
            for segment in url.path_segments().expect("FIXME") {
                file_path.push(segment);
            }
            // TODO: handle query
            // HINT: Use characters which must be percent-encoded in path as separator between path and query
            // HINT: handle extension
        })
        .unwrap_or_else(|err| {
            warn!("Failed to parse url: {} {}", &url, err);
            let ext = url
                .rsplit('.')
                .next()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| String::from("png"));
            file_path.push(format!("{}.{}", GUID::rand(), ext));
        });
    file_path
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
            if let Some(err) = media.delete(conn).err() {
                match &err {
                    Error::Io(e) => match e.kind() {
                        std::io::ErrorKind::NotFound => (),
                        _ => panic!("{:?}", err),
                    },
                    _ => panic!("{:?}", err),
                }
            }
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
