use rocket::{
    http::uri::{FromUriParam, UriDisplay},
    response::NamedFile
};
use std::{
    fmt,
    path::{Path, PathBuf}
};

macro_rules! may_fail {
    ($account:expr, $expr:expr, $template:expr, $msg:expr, | $res:ident | $block:block) => {
        {
            let res = $expr;
            if res.is_some() {
                let $res = res.unwrap();
                $block
            } else {
                Template::render(concat!("errors/", $template), json!({
                    "error_message": $msg,
                    "account": $account
                }))
            }
        }
    };
    ($account:expr, $expr:expr, $msg:expr, | $res:ident | $block:block) => {
        may_fail!($account, $expr, "404", $msg, |$res| {
            $block
        })
    };
    ($account:expr, $expr:expr, | $res:ident | $block:block) => {
        may_fail!($account, $expr, "", |$res| {
            $block
        })
    };
}

const ITEMS_PER_PAGE: i32 = 10;

#[derive(FromForm)]
pub struct Page {
    page: i32
}

impl UriDisplay for Page {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "page={}", &self.page as &UriDisplay)
    }
}

impl FromUriParam<i32> for Page {
    type Target = Page;
    fn from_uri_param(num: i32) -> Page {
        Page { page: num }
    }
}

impl Page {
    pub fn first() -> Page {
        Page {
            page: 1
        }
    }

    pub fn limits(&self) -> (i32, i32) {
        ((self.page - 1) * ITEMS_PER_PAGE, self.page * ITEMS_PER_PAGE)
    }
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
