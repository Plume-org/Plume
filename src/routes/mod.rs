use atom_syndication::{ContentBuilder, Entry, EntryBuilder, LinkBuilder, Person, PersonBuilder};
use rocket::{
    http::uri::{FromUriParam, UriDisplay},
    response::NamedFile
};
use std::{
    fmt,
    path::{Path, PathBuf}
};

use plume_models::{Connection, posts::Post};

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

const ITEMS_PER_PAGE: i32 = 12;

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

    /// Computes the total number of pages needed to display n_items
    pub fn total(n_items: i32) -> i32 {
        if n_items % ITEMS_PER_PAGE == 0 {
            n_items / ITEMS_PER_PAGE
        } else {
            (n_items / ITEMS_PER_PAGE) + 1
        }
    }

    pub fn limits(&self) -> (i32, i32) {
        ((self.page - 1) * ITEMS_PER_PAGE, self.page * ITEMS_PER_PAGE)
    }
}

pub fn post_to_atom(post: Post, conn: &Connection) -> Entry {
    EntryBuilder::default()
        .title(post.title.clone())
        .content(ContentBuilder::default()
            .value(format!("<![CDATA[{}]]>", *post.content.get()))
            .src(post.ap_url.clone())
            .content_type("html".to_string())
            .build().expect("Atom feed: content error"))
        .authors(post.get_authors(&*conn)
            .into_iter()
            .map(|a| PersonBuilder::default()
                .name(a.display_name)
                .uri(a.ap_url)
                .build().expect("Atom feed: author error"))
            .collect::<Vec<Person>>())
        .links(vec![LinkBuilder::default().href(post.ap_url).build().expect("Atom feed: link error")])
        .build().expect("Atom feed: entry error")
}

pub mod blogs;
pub mod comments;
pub mod errors;
pub mod instance;
pub mod likes;
pub mod medias;
pub mod notifications;
pub mod posts;
pub mod reshares;
pub mod session;
pub mod tags;
pub mod user;
pub mod well_known;

#[get("/static/<file..>", rank = 2)]
fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
}
