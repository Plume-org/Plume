use guid_create::GUID;
use multipart::server::{Multipart, save::{SavedData, SaveResult}};
use rocket::{Data, http::ContentType, response::{NamedFile, Redirect}};
use rocket_contrib::Template;
use serde_json;
use std::{fs, path::{Path, PathBuf}};
use plume_models::{db_conn::DbConn, medias::*, users::User};

#[get("/medias")]
fn list(user: User, conn: DbConn) -> Template {
    let medias = Media::for_user(&*conn, user.id);
    Template::render("medias/index", json!({
        "account": user,
        "medias": medias.into_iter().map(|m| m.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/medias/new")]
fn new(user: User) -> Template {
    Template::render("medias/new", json!({
        "account": user,
        "form": {},
        "errors": {}
    }))
}

#[post("/medias/new", data = "<data>")]
fn upload(user: User, data: Data, ct: &ContentType, conn: DbConn) -> Redirect {
    if ct.is_form_data() {
        let (_, boundary) = ct.params().find(|&(k, _)| k == "boundary").expect("No boundary");

        match Multipart::with_body(data.open(), boundary).save().temp() {
            SaveResult::Full(entries) => {
                let fields = entries.fields;

                let filename = fields.get(&"file".to_string()).unwrap().into_iter().next().unwrap().headers
                    .filename.clone()
                    .unwrap_or("x.png".to_string()); // PNG by default
                let ext = filename.rsplitn(2, ".")
                    .next()
                    .unwrap();
                let dest = format!("media/{}.{}", GUID::rand().to_string(), ext);

                if let SavedData::Bytes(ref bytes) = fields[&"file".to_string()][0].data {
                    fs::write(&dest, bytes).expect("Couldn't save upload");
                } else {
                    if let SavedData::File(ref path, _) = fields[&"file".to_string()][0].data {
                        fs::copy(path, &dest).expect("Couldn't copy temp upload");
                    } else {
                        println!("not file");
                        return Redirect::to(uri!(new));
                    }
                }

                let has_cw = read(&fields[&"cw".to_string()][0].data).len() > 0;
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
                Redirect::to(uri!(details: id = media.id))
            },
            SaveResult::Partial(_, _) | SaveResult::Error(_) => {
                println!("partial err");
                Redirect::to(uri!(new))
            }
        }
    } else {
        println!("not form data");
        Redirect::to(uri!(new))
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
fn details(id: i32, user: User, conn: DbConn) -> Template {
    let media = Media::get(&*conn, id);
    Template::render("medias/details", json!({
        "account": user,
        "media": media.map(|m| m.to_json(&*conn))
    }))
}

#[get("/static/media/<file..>", rank = 1)]
fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("media/").join(file)).ok()
}

