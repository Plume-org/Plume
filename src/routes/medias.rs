use crate::routes::{errors::ErrorPage, Page};
use crate::template_utils::{IntoContext, Ructe};
use guid_create::GUID;
use plume_models::{db_conn::DbConn, medias::*, users::User, Error, PlumeRocket, CONFIG};
use rocket::{
    form::Form,
    fs::TempFile,
    response::{status, Flash, Redirect},
};
use rocket_i18n::I18n;

#[get("/medias?<page>")]
pub fn list(
    user: User,
    page: Option<Page>,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let page = page.unwrap_or_default();
    let medias = Media::page_for_user(&mut conn, &user, page.limits())?;
    let total_page = Page::total(Media::count_for_user(&mut conn, &user)? as i32);
    Ok(render!(medias::index_html(
        &(&mut conn, &rockets).to_context(),
        medias,
        page.0,
        total_page
    )))
}

#[get("/medias/new")]
pub fn new(_user: User, mut conn: DbConn, rockets: PlumeRocket) -> Ructe {
    render!(medias::new_html(&(&mut conn, &rockets).to_context()))
}

#[derive(FromForm)]
pub struct Upload<'r> {
    file: TempFile<'r>,
    cw: Option<String>,
    alt: String,
}

#[post("/medias/new", data = "<upload>")]
pub async fn upload(
    user: User,
    mut upload: Form<Upload<'_>>,
    mut conn: DbConn,
) -> Result<Redirect, status::BadRequest<&'static str>> {
    let file_path = match save_uploaded_file(&mut upload.file).await {
        Ok(Some(file_path)) => file_path,
        Ok(None) => return Ok(Redirect::to(uri!(new))),
        Err(_) => return Err(status::BadRequest("Couldn't save uploaded media: {}")),
    };

    let media = Media::insert(
        &mut conn,
        NewMedia {
            file_path,
            alt_text: upload.alt.clone(),
            is_remote: false,
            remote_url: None,
            sensitive: upload.cw.is_some(),
            content_warning: upload.cw.clone(),
            owner_id: user.id,
        },
    )
    .map_err(|_| status::BadRequest("Error while saving media"))?;
    Ok(Redirect::to(uri!(details(id = media.id))))
}

async fn save_uploaded_file<'r>(file: &mut TempFile<'r>) -> Result<Option<String>, plume_models::Error> {
    // Remove extension if it contains something else than just letters and numbers
    let ext = file
        .content_type()
        .map(|ct| ct.to_string().replace("/", "."))
        .unwrap_or_default();

    if CONFIG.s3.is_some() {
        #[cfg(not(feature="s3"))]
        unreachable!();

        #[cfg(feature="s3")]
        {
            use std::borrow::Cow;

            let dest = format!("static/media/{}.{}", GUID::rand(), ext);

            file.move_copy_to(&dest).await?;

            let bytes = Cow::from(std::fs::read(&dest)?);

            let bucket = CONFIG.s3.as_ref().unwrap().get_bucket();
            let content_type = match file.content_type() {
                Some(ct) => ct.to_string(),
                None => rocket::http::ContentType::from_extension(&ext)
                    .unwrap_or(rocket::http::ContentType::Binary)
                    .to_string(),
            };

            bucket.put_object_with_content_type_blocking(&dest, &bytes, &content_type)?;

            Ok(Some(dest))
        }
    } else {
        let dest = format!("{}/{}.{}", CONFIG.media_directory, GUID::rand(), ext);
        file.move_copy_to(&dest).await?;
        Ok(Some(dest))
    }
}

#[get("/medias/<id>")]
pub fn details(
    id: i32,
    user: User,
    mut conn: DbConn,
    rockets: PlumeRocket,
) -> Result<Ructe, ErrorPage> {
    let media = Media::get(&mut conn, id)?;
    if media.owner_id == user.id {
        Ok(render!(medias::details_html(
            &(&mut conn, &rockets).to_context(),
            media
        )))
    } else {
        Err(Error::Unauthorized.into())
    }
}

#[post("/medias/<id>/delete")]
pub fn delete(id: i32, user: User, mut conn: DbConn, intl: I18n) -> Result<Flash<Redirect>, ErrorPage> {
    let media = Media::get(&mut conn, id)?;
    if media.owner_id == user.id {
        media.delete(&mut conn)?;
        Ok(Flash::success(
            Redirect::to(uri!(list(page = _))),
            i18n!(intl.catalog, "Your media have been deleted."),
        ))
    } else {
        Ok(Flash::error(
            Redirect::to(uri!(list(page = _))),
            i18n!(intl.catalog, "You are not allowed to delete this media."),
        ))
    }
}

#[post("/medias/<id>/avatar")]
pub fn set_avatar(
    id: i32,
    user: User,
    mut conn: DbConn,
    intl: I18n,
) -> Result<Flash<Redirect>, ErrorPage> {
    let media = Media::get(&mut conn, id)?;
    if media.owner_id == user.id {
        user.set_avatar(&mut conn, media.id)?;
        Ok(Flash::success(
            Redirect::to(uri!(details(id = id))),
            i18n!(intl.catalog, "Your avatar has been updated."),
        ))
    } else {
        Ok(Flash::error(
            Redirect::to(uri!(details(id = id))),
            i18n!(intl.catalog, "You are not allowed to use this media."),
        ))
    }
}
