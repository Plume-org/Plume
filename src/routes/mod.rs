use rocket::response::NamedFile;
use std::path::{Path, PathBuf};

macro_rules! may_fail {
    ($expr:expr, $template:expr, $msg:expr, | $res:ident | $block:block) => {
        {
            let res = $expr;
            if res.is_some() {
                let $res = res.unwrap();
                $block
            } else {
                Template::render(concat!("errors/", stringify!($template)), json!({
                    "error_message": $msg
                }))
            }
        }
    };
    ($expr:expr, $msg:expr, | $res:ident | $block:block) => {
        may_fail!($expr, "404", $msg, |$res| {
            $block
        })
    };
    ($expr:expr, | $res:ident | $block:block) => {
        may_fail!($expr, "", |$res| {
            $block
        })
    };
}

pub mod blogs;
pub mod comments;
pub mod errors;
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
