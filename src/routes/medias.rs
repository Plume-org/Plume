use guid_create::GUID;
use multipart::server::{Multipart, save::{SavedData, SaveResult}};
use rocket::{Data, http::ContentType, response::{Redirect, status}};
use rocket_i18n::I18n;
use std::fs;
use plume_models::{db_conn::DbConn, medias::*, users::User};
use routes::Ructe;

#[get("/medias")]
pub fn list(user: User, conn: DbConn, intl: I18n) -> Ructe {
    let medias = Media::for_user(&*conn, user.id);
    render!(medias::index(
        (&*conn, &intl.catalog, Some(user)),
        medias
    ))
}

#[get("/medias/new")]
pub fn new(user: User, conn: DbConn, intl: I18n) -> Ructe {
    render!(medias::new(
        (&*conn, &intl.catalog, Some(user))
    ))
}

#[post("/medias/new", data = "<data>")]
pub fn upload(user: User, data: Data, ct: &ContentType, conn: DbConn) -> Result<Redirect, status::BadRequest<&'static str>> {
    if ct.is_form_data() {
        let (_, boundary) = ct.params().find(|&(k, _)| k == "boundary").ok_or_else(|| status::BadRequest(Some("No boundary")))?;

        match Multipart::with_body(data.open(), boundary).save().temp() {
            SaveResult::Full(entries) => {
                let fields = entries.fields;

                let filename = fields.get(&"file".to_string()).and_then(|v| v.into_iter().next())
                    .ok_or_else(|| status::BadRequest(Some("No file uploaded")))?.headers
                    .filename.clone();
                let ext = filename.and_then(|f| f.rsplit('.').next().map(|ext| ext.to_owned()))
                    .unwrap_or_else(|| "png".to_owned());
                let dest = format!("static/media/{}.{}", GUID::rand().to_string(), ext);

                match fields[&"file".to_string()][0].data {
                    SavedData::Bytes(ref bytes) => fs::write(&dest, bytes).expect("media::upload: Couldn't save upload"),
                    SavedData::File(ref path, _) => {fs::copy(path, &dest).expect("media::upload: Couldn't copy upload");},
                    _ => {
                        println!("not a file");
                        return Ok(Redirect::to(uri!(new)));
                    }
                }

                let has_cw = !read(&fields[&"cw".to_string()][0].data).is_empty();
                let media = Media::insert(&*conn, NewMedia {
                    file_path: dest,
                    alt_text: read(&fields[&"alt".to_string()][0].data),
                    is_remote: false,
                    remote_url: None,
                    sensitive: has_cw,
                    content_warning: if has_cw {
                        Some(read(&fields[&"cw".to_string()][0].data))
                    } else {
                        None
                    },
                    owner_id: user.id
                });
                println!("ok");
                Ok(Redirect::to(uri!(details: id = media.id)))
            },
            SaveResult::Partial(_, _) | SaveResult::Error(_) => {
                println!("partial err");
                Ok(Redirect::to(uri!(new)))
            }
        }
    } else {
        println!("not form data");
        Ok(Redirect::to(uri!(new)))
    }
}

fn read(data: &SavedData) -> String {
    if let SavedData::Text(s) = data {
        s.clone()
    } else {
        panic!("Field is not a string")
    }
}

#[get("/medias/<id>")]
pub fn details(id: i32, user: User, conn: DbConn, intl: I18n) -> Ructe {
    let media = Media::get(&*conn, id).expect("Media::details: media not found");
    render!(medias::details(
        (&*conn, &intl.catalog, Some(user)),
        media
    ))
}

#[post("/medias/<id>/delete")]
pub fn delete(id: i32, _user: User, conn: DbConn) -> Option<Redirect> {
    let media = Media::get(&*conn, id)?;
    media.delete(&*conn);
    Some(Redirect::to(uri!(list)))
}

#[post("/medias/<id>/avatar")]
pub fn set_avatar(id: i32, user: User, conn: DbConn) -> Option<Redirect> {
    let media = Media::get(&*conn, id)?;
    user.set_avatar(&*conn, media.id);
    Some(Redirect::to(uri!(details: id = id)))
}
