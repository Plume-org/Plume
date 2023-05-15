#![warn(clippy::too_many_arguments)]
use crate::template_utils::Ructe;
use atom_syndication::{
    ContentBuilder, Entry, EntryBuilder, Feed, FeedBuilder, LinkBuilder, Person, PersonBuilder,
};
use chrono::{naive::NaiveDateTime, DateTime, Utc};
use plume_models::{posts::Post, Connection, CONFIG, ITEMS_PER_PAGE};
use rocket::{
    http::{
        hyper::header::{CacheControl, CacheDirective, ETag, EntityTag},
        uri::{FromUriParam, Query},
        RawStr, Status,
    },
    request::{self, FromFormValue, FromRequest, Request},
    response::{self, Flash, NamedFile, Redirect, Responder, Response},
    Outcome,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::Hasher,
    path::{Path, PathBuf},
};

#[cfg(feature = "s3")]
use rocket::http::ContentType;

/// Special return type used for routes that "cannot fail", and instead
/// `Redirect`, or `Flash<Redirect>`, when we cannot deliver a `Ructe` Response
#[allow(clippy::large_enum_variant)]
#[derive(Responder)]
pub enum RespondOrRedirect {
    Response(Ructe),
    FlashResponse(Flash<Ructe>),
    Redirect(Redirect),
    FlashRedirect(Flash<Redirect>),
}

impl From<Ructe> for RespondOrRedirect {
    fn from(response: Ructe) -> Self {
        RespondOrRedirect::Response(response)
    }
}

impl From<Flash<Ructe>> for RespondOrRedirect {
    fn from(response: Flash<Ructe>) -> Self {
        RespondOrRedirect::FlashResponse(response)
    }
}

impl From<Redirect> for RespondOrRedirect {
    fn from(redirect: Redirect) -> Self {
        RespondOrRedirect::Redirect(redirect)
    }
}

impl From<Flash<Redirect>> for RespondOrRedirect {
    fn from(redirect: Flash<Redirect>) -> Self {
        RespondOrRedirect::FlashRedirect(redirect)
    }
}

#[derive(Shrinkwrap, Copy, Clone, UriDisplayQuery)]
pub struct Page(i32);

impl From<i32> for Page {
    fn from(page: i32) -> Self {
        Self(page)
    }
}

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

#[derive(Shrinkwrap)]
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

/// A form for remote interaction, used by multiple routes
#[derive(Shrinkwrap, Clone, Default, FromForm)]
pub struct RemoteForm {
    pub remote: String,
}

pub fn build_atom_feed(
    entries: Vec<Post>,
    uri: &str,
    title: &str,
    default_updated: &NaiveDateTime,
    conn: &Connection,
) -> Feed {
    let updated = if entries.is_empty() {
        default_updated
    } else {
        &entries[0].creation_date
    };

    FeedBuilder::default()
        .title(title)
        .id(uri)
        .updated(DateTime::<Utc>::from_utc(*updated, Utc))
        .entries(
            entries
                .into_iter()
                .map(|p| post_to_atom(p, conn))
                .collect::<Vec<Entry>>(),
        )
        .links(vec![LinkBuilder::default()
            .href(uri)
            .rel("self")
            .mime_type("application/atom+xml".to_string())
            .build()])
        .build()
}

fn post_to_atom(post: Post, conn: &Connection) -> Entry {
    EntryBuilder::default()
        .title(format!("<![CDATA[{}]]>", post.title))
        .content(
            ContentBuilder::default()
                .value(format!("<![CDATA[{}]]>", *post.content.get()))
                .content_type("html".to_string())
                .build(),
        )
        .authors(
            post.get_authors(conn)
                .expect("Atom feed: author error")
                .into_iter()
                .map(|a| {
                    PersonBuilder::default()
                        .name(a.display_name)
                        .uri(a.ap_url)
                        .build()
                })
                .collect::<Vec<Person>>(),
        )
        // Using RFC 4287 format, see https://tools.ietf.org/html/rfc4287#section-3.3 for dates
        // eg: 2003-12-13T18:30:02Z (Z is here because there is no timezone support with the NaiveDateTime crate)
        .published(Some(
            DateTime::<Utc>::from_utc(post.creation_date, Utc).into(),
        ))
        .updated(DateTime::<Utc>::from_utc(post.creation_date, Utc))
        .id(post.ap_url.clone())
        .links(vec![LinkBuilder::default().href(post.ap_url).build()])
        .build()
}

pub mod blogs;
pub mod comments;
pub mod email_signups;
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
pub mod timelines;
pub mod user;
pub mod well_known;

#[derive(Responder)]
enum FileKind {
    Local(NamedFile),
    #[cfg(feature = "s3")]
    S3(Vec<u8>, ContentType),
}

#[derive(Responder)]
#[response()]
pub struct CachedFile {
    inner: FileKind,
    cache_control: CacheControl,
}

#[derive(Debug)]
pub struct ThemeFile(NamedFile);

impl<'r> Responder<'r> for ThemeFile {
    fn respond_to(self, r: &Request<'_>) -> response::Result<'r> {
        let contents = std::fs::read(self.0.path()).map_err(|_| Status::InternalServerError)?;

        let mut hasher = DefaultHasher::new();
        hasher.write(&contents);
        let etag = format!("{:x}", hasher.finish());

        if r.headers()
            .get("If-None-Match")
            .any(|s| s[1..s.len() - 1] == etag)
        {
            Response::build()
                .status(Status::NotModified)
                .header(ETag(EntityTag::strong(etag)))
                .ok()
        } else {
            Response::build()
                .merge(self.0.respond_to(r)?)
                .header(ETag(EntityTag::strong(etag)))
                .ok()
        }
    }
}

#[get("/static/cached/<_build_id>/css/<file..>", rank = 1)]
pub fn theme_files(file: PathBuf, _build_id: &RawStr) -> Option<ThemeFile> {
    NamedFile::open(Path::new("static/css/").join(file))
        .ok()
        .map(ThemeFile)
}

#[allow(unused_variables)]
#[get("/static/cached/<build_id>/<file..>", rank = 2)]
pub fn plume_static_files(file: PathBuf, build_id: &RawStr) -> Option<CachedFile> {
    static_files(file)
}
#[get("/static/media/<file..>")]
pub fn plume_media_files(file: PathBuf) -> Option<CachedFile> {
    if CONFIG.s3.is_some() {
        #[cfg(not(feature="s3"))]
        unreachable!();

        #[cfg(feature="s3")]
        {
            let data = CONFIG.s3.as_ref().unwrap().get_bucket()
                .get_object_blocking(format!("static/media/{}", file.to_string_lossy())).ok()?;

            let ct = data.headers().get("content-type")
                .and_then(|x| ContentType::parse_flexible(&x))
                .or_else(|| file.extension()
                    .and_then(|ext| ContentType::from_extension(&ext.to_string_lossy())))
                .unwrap_or(ContentType::Binary);

            Some(CachedFile {
                inner: FileKind::S3(data.to_vec(), ct),
                cache_control: CacheControl(vec![CacheDirective::MaxAge(60 * 60 * 24 * 30)]),
            })
        }
    } else {
        NamedFile::open(Path::new(&CONFIG.media_directory).join(file))
            .ok()
            .map(|f| CachedFile {
                inner: FileKind::Local(f),
                cache_control: CacheControl(vec![CacheDirective::MaxAge(60 * 60 * 24 * 30)]),
            })
    }
}
#[get("/static/<file..>", rank = 3)]
pub fn static_files(file: PathBuf) -> Option<CachedFile> {
    NamedFile::open(Path::new("static/").join(file))
        .ok()
        .map(|f| CachedFile {
            inner: FileKind::Local(f),
            cache_control: CacheControl(vec![CacheDirective::MaxAge(60 * 60 * 24 * 30)]),
        })
}
