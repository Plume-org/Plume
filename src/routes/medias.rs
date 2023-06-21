use crate::routes::{errors::ErrorPage, Page};
use crate::template_utils::{IntoContext, Ructe};
use guid_create::GUID;
use multipart::server::{
    save::{SaveResult, SavedField, SavedData},
    Multipart,
};
use plume_models::{db_conn::DbConn, medias::*, users::User, Error, PlumeRocket, CONFIG};
use rocket::{
    http::ContentType,
    response::{status, Flash, Redirect},
    Data,
};
use rocket_i18n::I18n;
use std::fs;

#[get("/medias?<page>")]
pub fn list(
    user: User,
    page: Option<Page>,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let medias = Media::page_for_user(&conn, &user, page.limits())?;
    Ok(render!(medias::index(
        &(&conn, &rockets).to_context(),
        medias,
        page.0,
        Page::total(Media::count_for_user(&conn, &user)? as i32)
    )))
}

#[get("/medias/new")]
pub fn new(_user: User, conn: DbConn, rockets: PlumeRocket) -> Ructe {
    render!(medias::new(&(&conn, &rockets).to_context()))
}

#[post("/medias/new", data = "<data>")]
pub fn upload(
    user: User,
    data: Data,
    ct: &ContentType,
    conn: DbConn,
) -> Result<Redirect, status::BadRequest<&'static str>> {
    if !ct.is_form_data() {
        return Ok(Redirect::to(uri!(new)));
    }

    let (_, boundary) = ct
        .params()
        .find(|&(k, _)| k == "boundary")
        .ok_or(status::BadRequest(Some("No boundary")))?;

    if let SaveResult::Full(entries) = Multipart::with_body(data.open(), boundary).save().temp() {
        let fields = entries.fields;

        let file = fields
            .get("file")
            .and_then(|v| v.iter().next())
            .ok_or(status::BadRequest(Some("No file uploaded")))?;

        let file_path = match save_uploaded_file(file) {
            Ok(Some(file_path)) => file_path,
            Ok(None) => return Ok(Redirect::to(uri!(new))),
            Err(_) => return Err(status::BadRequest(Some("Couldn't save uploaded media: {}"))),
        };

        let has_cw = !read(&fields["cw"][0].data)
            .map(|cw| cw.is_empty())
            .unwrap_or(false);
        let media = Media::insert(
            &conn,
            NewMedia {
                file_path,
                alt_text: read(&fields["alt"][0].data)?,
                is_remote: false,
                remote_url: None,
                sensitive: has_cw,
                content_warning: if has_cw {
                    Some(read(&fields["cw"][0].data)?)
                } else {
                    None
                },
                owner_id: user.id,
            },
        )
        .map_err(|_| status::BadRequest(Some("Error while saving media")))?;
        Ok(Redirect::to(uri!(details: id = media.id)))
    } else {
        Ok(Redirect::to(uri!(new)))
    }
}

fn save_uploaded_file(file: &SavedField) -> Result<Option<String>, plume_models::Error> {
    // Remove extension if it contains something else than just letters and numbers
    let ext = file
        .headers
        .filename
        .as_ref()
        .and_then(|f| {
            f.rsplit('.')
                .next()
                .and_then(|ext| {
                    if ext.chars().any(|c| !c.is_alphanumeric()) {
                        None
                    } else {
                        Some(ext.to_lowercase())
                    }
                })
        })
        .unwrap_or_default();

    if CONFIG.s3.is_some() {
        #[cfg(not(feature="s3"))]
        unreachable!();

        #[cfg(feature="s3")]
        {
            use std::borrow::Cow;

            let dest = format!("static/media/{}.{}", GUID::rand(), ext);

            let bytes = match file.data {
                SavedData::Bytes(ref bytes) => Cow::from(bytes),
                SavedData::File(ref path, _) => Cow::from(fs::read(path)?),
                _ => {
                    return Ok(None);
                }
            };

            let bucket = CONFIG.s3.as_ref().unwrap().get_bucket();
            let content_type = match &file.headers.content_type {
                Some(ct) => ct.to_string(),
                None => ContentType::from_extension(&ext)
                    .unwrap_or(ContentType::Binary)
                    .to_string(),
            };

            bucket.put_object_with_content_type_blocking(&dest, &bytes, &content_type)?;

            Ok(Some(dest))
        }
    } else {
        let dest = format!("{}/{}.{}", CONFIG.media_directory, GUID::rand(), ext);

        match file.data {
            SavedData::Bytes(ref bytes) => {
                fs::write(&dest, bytes)?;
            }
            SavedData::File(ref path, _) => {
                fs::copy(path, &dest)?;
            }
            _ => {
                return Ok(None);
            }
        }

        Ok(Some(dest))
    }
}

fn read(data: &SavedData) -> Result<String, status::BadRequest<&'static str>> {
    if let SavedData::Text(s) = data {
        Ok(s.clone())
    } else {
        Err(status::BadRequest(Some("Error while reading data")))
    }
}

#[get("/medias/<id>")]
pub fn details(
    id: i32,
    user: User,
    conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let media = Media::get(&conn, id)?;
    if media.owner_id == user.id {
        Ok(render!(medias::details(
            &(&conn, &rockets).to_context(),
            media
        )))
    } else {
        Err(Error::Unauthorized.into())
    }
}

#[post("/medias/<id>/delete")]
pub fn delete(id: i32, user: User, conn: DbConn, intl: I18n) -> Result<Flash<Redirect>, ErrorPage> {
    let media = Media::get(&conn, id)?;
    if media.owner_id == user.id {
        media.delete(&conn)?;
        Ok(Flash::success(
            Redirect::to(uri!(list: page = _)),
            i18n!(intl.catalog, "Your media have been deleted."),
        ))
    } else {
        Ok(Flash::error(
            Redirect::to(uri!(list: page = _)),
            i18n!(intl.catalog, "You are not allowed to delete this media."),
        ))
    }
}

#[post("/medias/<id>/avatar")]
pub fn set_avatar(
    id: i32,
    user: User,
    conn: DbConn,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    let media = Media::get(&conn, id)?;
    if media.owner_id == user.id {
        user.set_avatar(&conn, media.id)?;
        Ok(Flash::success(
            Redirect::to(uri!(details: id = id)),
            i18n!(intl.catalog, "Your avatar has been updated."),
        ))
    } else {
        Ok(Flash::error(
            Redirect::to(uri!(details: id = id)),
            i18n!(intl.catalog, "You are not allowed to use this media."),
        ))
    }
}
