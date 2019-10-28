use guid_create::GUID;
use multipart::server::{
    save::{SaveResult, SavedData},
    Multipart,
};
use plume_models::{db_conn::DbConn, medias::*, users::User, Error, PlumeRocket, CONFIG};
use rocket::{
    http::ContentType,
    response::{status, Flash, Redirect},
    Data,
};
use rocket_i18n::I18n;
use routes::{errors::ErrorPage, Page};
use std::fs;
use template_utils::{IntoContext, Ructe};

#[get("/medias?<page>")]
pub fn list(user: User, page: Option<Page>, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let medias = Media::page_for_user(&*rockets.conn, &user, page.limits())?;
    Ok(render!(medias::index(
        &rockets.to_context(),
        medias,
        page.0,
        Page::total(Media::count_for_user(&*rockets.conn, &user)? as i32)
    )))
}

#[get("/medias/new")]
pub fn new(_user: User, rockets: PlumeRocket) -> Ructe {
    render!(medias::new(&rockets.to_context()))
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
        .ok_or_else(|| status::BadRequest(Some("No boundary")))?;

    if let SaveResult::Full(entries) = Multipart::with_body(data.open(), boundary).save().temp() {
        let fields = entries.fields;

        let filename = fields
            .get("file")
            .and_then(|v| v.iter().next())
            .ok_or_else(|| status::BadRequest(Some("No file uploaded")))?
            .headers
            .filename
            .clone();
        // Remove extension if it contains something else than just letters and numbers
        let ext = filename
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
                    .map(|ext| format!(".{}", ext))
            })
            .unwrap_or_default();
        let dest = format!(
            "{}/{}{}",
            CONFIG.media_directory,
            GUID::rand().to_string(),
            ext
        );

        match fields["file"][0].data {
            SavedData::Bytes(ref bytes) => fs::write(&dest, bytes)
                .map_err(|_| status::BadRequest(Some("Couldn't save upload")))?,
            SavedData::File(ref path, _) => {
                fs::copy(path, &dest)
                    .map_err(|_| status::BadRequest(Some("Couldn't copy upload")))?;
            }
            _ => {
                return Ok(Redirect::to(uri!(new)));
            }
        }

        let has_cw = !read(&fields["cw"][0].data)
            .map(|cw| cw.is_empty())
            .unwrap_or(false);
        let media = Media::insert(
            &*conn,
            NewMedia {
                file_path: dest,
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

fn read(data: &SavedData) -> Result<String, status::BadRequest<&'static str>> {
    if let SavedData::Text(s) = data {
        Ok(s.clone())
    } else {
        Err(status::BadRequest(Some("Error while reading data")))
    }
}

#[get("/medias/<id>")]
pub fn details(id: i32, user: User, rockets: PlumeRocket) -> Result<Ructe, ErrorPage> {
    let media = Media::get(&*rockets.conn, id)?;
    if media.owner_id == user.id {
        Ok(render!(medias::details(&rockets.to_context(), media)))
    } else {
        Err(Error::Unauthorized.into())
    }
}

#[post("/medias/<id>/delete")]
pub fn delete(id: i32, user: User, conn: DbConn, intl: I18n) -> Result<Flash<Redirect>, ErrorPage> {
    let media = Media::get(&*conn, id)?;
    if media.owner_id == user.id {
        media.delete(&*conn)?;
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
    let media = Media::get(&*conn, id)?;
    if media.owner_id == user.id {
        user.set_avatar(&*conn, media.id)?;
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
