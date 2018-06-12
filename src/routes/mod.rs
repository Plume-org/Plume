use rocket::response::NamedFile;
use std::path::{Path, PathBuf};

pub mod blogs;
pub mod comments;
pub mod instance;
pub mod likes;
pub mod notifications;
pub mod posts;
pub mod reshares;
pub mod session;
pub mod user;
pub mod well_known;

#[get("/static/<file..>")]
fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
}
