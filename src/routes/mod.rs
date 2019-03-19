#![warn(clippy::too_many_arguments)]
use atom_syndication::{ContentBuilder, Entry, EntryBuilder, LinkBuilder, Person, PersonBuilder};
use rocket::{
    http::{
        hyper::header::{CacheControl, CacheDirective},
        uri::{FromUriParam, Query},
        RawStr, Status,
    },
    request::{self, FromFormValue, FromRequest, Request},
    response::NamedFile,
    Outcome,
};
use rocket_i18n::I18n;
use std::path::{Path, PathBuf};

use plume_models::{db_conn::DbConn, posts::Post, users::User, Connection};

use Searcher;
use Worker;

pub struct PlumeRocket<'a> {
    conn: DbConn,
    intl: I18n,
    user: Option<User>,
    searcher: Searcher<'a>,
    worker: Worker<'a>,
}

impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket<'a> {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<PlumeRocket<'a>, ()> {
        let conn = request.guard::<DbConn>()?;
        let intl = request.guard::<I18n>()?;
        let user = request.guard::<User>().succeeded();
        let worker = request.guard::<Worker>()?;
        let searcher = request.guard::<Searcher>()?;
        rocket::Outcome::Success(PlumeRocket {
            conn,
            intl,
            user,
            worker,
            searcher,
        })
    }
}

const ITEMS_PER_PAGE: i32 = 12;

#[derive(Copy, Clone, UriDisplayQuery)]
pub struct Page(i32);

impl<'v> FromFormValue<'v> for Page {
    type Error = &'v RawStr;
    fn from_form_value(form_value: &'v RawStr) -> Result<Page, &'v RawStr> {
        match form_value.parse::<i32>() {
            Ok(page) => Ok(Page(page)),
            _ => Err(form_value),
        }
    }
}

impl FromUriParam<Query, Option<Page>> for Page {
    type Target = Page;

    fn from_uri_param(val: Option<Page>) -> Page {
        val.unwrap_or_default()
    }
}

impl Page {
    /// Computes the total number of pages needed to display n_items
    pub fn total(n_items: i32) -> i32 {
        if n_items % ITEMS_PER_PAGE == 0 {
            n_items / ITEMS_PER_PAGE
        } else {
            (n_items / ITEMS_PER_PAGE) + 1
        }
    }

    pub fn limits(self) -> (i32, i32) {
        ((self.0 - 1) * ITEMS_PER_PAGE, self.0 * ITEMS_PER_PAGE)
    }
}

pub struct ContentLen(pub u64);

impl<'a, 'r> FromRequest<'a, 'r> for ContentLen {
    type Error = ();

    fn from_request(r: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        match r.limits().get("forms") {
            Some(l) => Outcome::Success(ContentLen(l)),
            None => Outcome::Failure((Status::InternalServerError, ())),
        }
    }
}

impl Default for Page {
    fn default() -> Self {
        Page(1)
    }
}

pub fn post_to_atom(post: Post, conn: &Connection) -> Entry {
    EntryBuilder::default()
        .title(format!("<![CDATA[{}]]>", post.title))
        .content(
            ContentBuilder::default()
                .value(format!("<![CDATA[{}]]>", *post.content.get()))
                .src(post.ap_url.clone())
                .content_type("html".to_string())
                .build()
                .expect("Atom feed: content error"),
        )
        .authors(
            post.get_authors(&*conn)
                .expect("Atom feed: author error")
                .into_iter()
                .map(|a| {
                    PersonBuilder::default()
                        .name(a.display_name)
                        .uri(a.ap_url)
                        .build()
                        .expect("Atom feed: author error")
                })
                .collect::<Vec<Person>>(),
        )
        .links(vec![LinkBuilder::default()
            .href(post.ap_url)
            .build()
            .expect("Atom feed: link error")])
        .build()
        .expect("Atom feed: entry error")
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
pub mod search;
pub mod session;
pub mod tags;
pub mod user;
pub mod well_known;

#[derive(Responder)]
#[response()]
pub struct CachedFile {
    inner: NamedFile,
    cache_control: CacheControl,
}

#[get("/static/cached/<_build_id>/<file..>", rank = 2)]
pub fn plume_static_files(file: PathBuf, _build_id: &RawStr) -> Option<CachedFile> {
    static_files(file)
}

#[get("/static/<file..>", rank = 3)]
pub fn static_files(file: PathBuf) -> Option<CachedFile> {
    NamedFile::open(Path::new("static/").join(file))
        .ok()
        .map(|f| CachedFile {
            inner: f,
            cache_control: CacheControl(vec![CacheDirective::MaxAge(60 * 60 * 24 * 30)]),
        })
}
