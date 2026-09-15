use crate::{
    ap_url, instance::Instance, safe_string::SafeString, schema::medias, users::User, Connection,
    Error, Result, CONFIG,
};
use activitystreams::{object::Image, prelude::*};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use guid_create::GUID;
use plume_common::{
    activity_pub::{inbox::FromId, request, ToAsString, ToAsUri},
    utils::{escape, MediaProcessor},
};
use std::{
    fs::{self, DirBuilder},
    path::{self, Path, PathBuf},
};
use tracing::warn;
use url::Url;

#[cfg(feature = "s3")]
use crate::config::S3Config;

const REMOTE_MEDIA_DIRECTORY: &str = "remote";

/// File extensions that are allowed for uploaded and mirrored media.
///
/// This is a whitelist of file types that can not be interpreted as active
/// content by browsers (no HTML, SVG, ...): serving user controlled files
/// with such extensions would allow stored XSS on the instance domain.
pub const ALLOWED_MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "mp3", "wav", "flac", "ogg", "oga", "mp4", "webm",
    "mov", "m4a", "m4v",
];

pub fn is_allowed_media_extension(ext: &str) -> bool {
    ALLOWED_MEDIA_EXTENSIONS
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(ext))
}

/// Returns the extension of the last path segment of `path` if it is allowed,
/// or `png` as a safe fallback.
fn safe_extension(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    ALLOWED_MEDIA_EXTENSIONS
        .iter()
        .find(|allowed| **allowed == ext)
        .copied()
        .unwrap_or("png")
}

/// Returns the segment if it can safely be used as a single path component
/// while mirroring a remote media.
fn sanitize_path_segment(segment: &str) -> Option<&str> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return None;
    }
    if segment.contains(['/', '\\', ':', '\0']) {
        return None;
    }
    Some(segment)
}

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

#[derive(PartialEq, Eq)]
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
            .unwrap_or("")
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
        let url = escape(&url);
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

    /// Returns full file path for medias stored in the local media directory.
    pub fn local_path(&self) -> Option<PathBuf> {
        if self.file_path.is_empty() {
            return None;
        }

        if CONFIG.s3.is_some() {
            #[cfg(feature = "s3")]
            unreachable!("Called Media::local_path() but media are stored on S3");
            #[cfg(not(feature = "s3"))]
            unreachable!();
        }

        let relative_path = self
            .file_path
            .trim_start_matches(&CONFIG.media_directory)
            .trim_start_matches(path::MAIN_SEPARATOR)
            .trim_start_matches("static/media/");

        Some(Path::new(&CONFIG.media_directory).join(relative_path))
    }

    /// Returns the relative URL to access this file, which is also the key at which
    /// it is stored in the S3 bucket if we are using S3 storage.
    /// Does not start with a '/', it is of the form "static/media/<...>"
    pub fn relative_url(&self) -> Option<String> {
        if self.file_path.is_empty() {
            return None;
        }

        let relative_path = self
            .file_path
            .trim_start_matches(&CONFIG.media_directory)
            .replace(path::MAIN_SEPARATOR, "/");

        let relative_path = relative_path
            .trim_start_matches('/')
            .trim_start_matches("static/media/");

        Some(format!("static/media/{}", relative_path))
    }

    /// Returns a public URL through which this media file can be accessed
    pub fn url(&self) -> Result<String> {
        if self.is_remote {
            Ok(self.remote_url.clone().unwrap_or_default())
        } else {
            let relative_url = self.relative_url().unwrap_or_default();

            #[cfg(feature = "s3")]
            if CONFIG
                .s3
                .as_ref()
                .map(|x| x.direct_download)
                .unwrap_or(false)
            {
                let s3_url = match CONFIG.s3.as_ref().unwrap() {
                    S3Config {
                        alias: Some(alias), ..
                    } => {
                        format!("https://{}/{}", alias, relative_url)
                    }
                    S3Config {
                        path_style: true,
                        hostname,
                        bucket,
                        ..
                    } => {
                        format!("https://{}/{}/{}", hostname, bucket, relative_url)
                    }
                    S3Config {
                        path_style: false,
                        hostname,
                        bucket,
                        ..
                    } => {
                        format!("https://{}.{}/{}", bucket, hostname, relative_url)
                    }
                };
                return Ok(s3_url);
            }

            Ok(ap_url(&format!(
                "{}/{}",
                Instance::get_local()?.public_domain,
                relative_url
            )))
        }
    }

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        if !self.is_remote {
            if CONFIG.s3.is_some() {
                #[cfg(not(feature = "s3"))]
                unreachable!();

                #[cfg(feature = "s3")]
                CONFIG
                    .s3
                    .as_ref()
                    .unwrap()
                    .get_bucket()
                    .delete_object_blocking(&self.relative_url().ok_or(Error::NotFound)?)?;
            } else {
                fs::remove_file(self.local_path().ok_or(Error::NotFound)?)?;
            }
        }
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn save_remote(conn: &Connection, url: String, user: &User) -> Result<Media> {
        let parsed = Url::parse(&url).map_err(|_| Error::Url)?;
        if !matches!(parsed.scheme(), "http" | "https") || !parsed.has_host() {
            return Err(Error::Url);
        }
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
        let remote_url = image
            .url()
            .and_then(|url| url.to_as_uri())
            .ok_or(Error::MissingApProperty)?;

        let file_path = if CONFIG.s3.is_some() {
            #[cfg(not(feature = "s3"))]
            unreachable!();

            #[cfg(feature = "s3")]
            {
                use rocket::http::ContentType;

                let dest = determine_mirror_s3_path(&remote_url);

                let media = request::get(
                    remote_url.as_str(),
                    User::get_sender(),
                    CONFIG.proxy().cloned(),
                )?;

                // Never trust the Content-Type announced by the remote
                // server: derive it from the (whitelisted) file extension so
                // that active content can not be served from the bucket.
                let content_type = dest
                    .rsplit('.')
                    .next()
                    .and_then(ContentType::from_extension)
                    .unwrap_or(ContentType::Binary);

                let bytes = media.bytes()?;

                let bucket = CONFIG.s3.as_ref().unwrap().get_bucket();
                bucket.put_object_with_content_type_blocking(
                    &dest,
                    &bytes,
                    &content_type.to_string(),
                )?;

                dest
            }
        } else {
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
            path.to_str().ok_or(Error::InvalidValue)?.to_string()
        };

        Media::find_by_file_path(conn, &file_path)
            .and_then(|mut media| {
                let mut updated = false;

                let alt_text = image
                    .content()
                    .and_then(|content| content.to_as_string())
                    .ok_or(Error::NotFound)?;
                let summary = image.summary().and_then(|summary| summary.to_as_string());
                let sensitive = summary.is_some();
                let content_warning = summary;
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
                    diesel::update(&media).set(&media).execute(conn)?;
                }
                Ok(media)
            })
            .or_else(|_| {
                let summary = image.summary().and_then(|summary| summary.to_as_string());
                Media::insert(
                    conn,
                    NewMedia {
                        file_path,
                        alt_text: image
                            .content()
                            .and_then(|content| content.to_as_string())
                            .ok_or(Error::NotFound)?,
                        is_remote: false,
                        remote_url: None,
                        sensitive: summary.is_some(),
                        content_warning: summary,
                        owner_id: User::from_id(
                            conn,
                            &image
                                .attributed_to()
                                .and_then(|attributed_to| attributed_to.to_as_uri())
                                .ok_or(Error::MissingApProperty)?,
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
    let mut file_path = Path::new(&CONFIG.media_directory).join(REMOTE_MEDIA_DIRECTORY);

    match Url::parse(url) {
        Ok(url) if url.has_host() => {
            if let Some(host) = url.host_str() {
                file_path.push(sanitize_path_segment(host).unwrap_or("unknown-host"));
            }
            for segment in url.path_segments().expect("FIXME") {
                if let Some(segment) = sanitize_path_segment(segment) {
                    file_path.push(segment);
                }
            }
            // Never trust the extension of a remote URL: it could be used to
            // make us serve active content (e.g. HTML) from our own domain.
            let ext = safe_extension(url.path());
            file_path.set_extension(ext);
            // TODO: handle query
            // HINT: Use characters which must be percent-encoded in path as separator between path and query
        }
        other => {
            if let Err(err) = other {
                warn!("Failed to parse url: {} {}", &url, err);
            } else {
                warn!("Error without a host: {}", &url);
            }
            file_path.push(format!("{}.png", GUID::rand()));
        }
    }
    file_path
}

#[cfg(feature = "s3")]
fn determine_mirror_s3_path(url: &str) -> String {
    match Url::parse(url) {
        Ok(url) if url.has_host() => {
            let mut path = format!(
                "static/media/{}/{}",
                REMOTE_MEDIA_DIRECTORY,
                url.host_str()
                    .and_then(sanitize_path_segment)
                    .unwrap_or("unknown-host"),
            );
            for segment in url.path_segments().expect("FIXME") {
                if let Some(segment) = sanitize_path_segment(segment) {
                    path.push('/');
                    path.push_str(segment);
                }
            }
            path.push('.');
            path.push_str(safe_extension(url.path()));
            path
        }
        other => {
            if let Err(err) = other {
                warn!("Failed to parse url: {} {}", &url, err);
            } else {
                warn!("Error without a host: {}", &url);
            }
            format!(
                "static/media/{}/{}.png",
                REMOTE_MEDIA_DIRECTORY,
                GUID::rand(),
            )
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{tests::db, users::tests as usersTests, Connection as Conn};
    use diesel::Connection;
    use std::env::{current_dir, set_current_dir};
    use std::fs;
    use std::path::{Component, Path};

    #[test]
    fn test_allowed_extensions() {
        assert!(is_allowed_media_extension("png"));
        assert!(is_allowed_media_extension("JPG"));
        assert!(!is_allowed_media_extension("html"));
        assert!(!is_allowed_media_extension("svg"));
        assert!(!is_allowed_media_extension(""));
        assert_eq!(safe_extension("/a/b/picture.PNG"), "png");
        assert_eq!(safe_extension("/a/b/evil.html"), "png");
        assert_eq!(safe_extension("/a/b/no-extension"), "png");
    }

    #[test]
    fn test_mirror_path_is_sanitized() {
        let base = Path::new(&CONFIG.media_directory).join(REMOTE_MEDIA_DIRECTORY);
        let path = determine_mirror_file_path("https://evil.example/../../../etc/passwd.html");
        assert!(path.starts_with(&base));
        assert!(!path.components().any(|c| matches!(c, Component::ParentDir)));
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));

        let path = determine_mirror_file_path("https://evil.example/a/pic.jpg");
        assert!(path.starts_with(&base));
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("jpg"));
    }

    #[test]
    fn test_save_remote_rejects_non_http_urls() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let users = fill_database(conn).0;
            assert!(Media::save_remote(conn, "javascript:alert(1)".to_owned(), &users[0]).is_err());
            assert!(Media::save_remote(conn, "file:///etc/passwd".to_owned(), &users[0]).is_err());
            assert!(
                Media::save_remote(conn, "https://example.com/pic.png".to_owned(), &users[0])
                    .is_ok()
            );
            clean(conn);
            Ok(())
        });
    }

    pub(crate) fn fill_database(conn: &Conn) -> (Vec<User>, Vec<Media>) {
        let mut wd = current_dir().unwrap();
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
                    file_path: path,
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
