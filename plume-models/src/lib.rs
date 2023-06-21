#![feature(never_type)]
#![feature(proc_macro_hygiene)]
#![feature(box_patterns)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate plume_macro;
#[macro_use]
extern crate rocket;
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate tantivy;

use activitystreams::iri_string;
pub use lettre;
pub use lettre::smtp;
use once_cell::sync::Lazy;
use plume_common::activity_pub::{inbox::InboxError, request, sign};
use posts::PostEvent;
use riker::actors::{channel, ActorSystem, ChannelRef, SystemBuilder};
use users::UserEvent;

#[cfg(not(any(feature = "sqlite", feature = "postgres")))]
compile_error!("Either feature \"sqlite\" or \"postgres\" must be enabled for this crate.");
#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("Either feature \"sqlite\" or \"postgres\" must be enabled for this crate.");

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
pub type Connection = diesel::SqliteConnection;

#[cfg(all(not(feature = "sqlite"), feature = "postgres"))]
pub type Connection = diesel::PgConnection;

pub(crate) static ACTOR_SYS: Lazy<ActorSystem> = Lazy::new(|| {
    SystemBuilder::new()
        .name("plume")
        .create()
        .expect("Failed to create actor system")
});

pub(crate) static USER_CHAN: Lazy<ChannelRef<UserEvent>> =
    Lazy::new(|| channel("user_events", &*ACTOR_SYS).expect("Failed to create user channel"));

pub(crate) static POST_CHAN: Lazy<ChannelRef<PostEvent>> =
    Lazy::new(|| channel("post_events", &*ACTOR_SYS).expect("Failed to create post channel"));

/// All the possible errors that can be encoutered in this crate
#[derive(Debug)]
pub enum Error {
    Blocklisted(bool, String),
    Db(diesel::result::Error),
    Inbox(Box<InboxError<Error>>),
    InvalidValue,
    Io(std::io::Error),
    MissingApProperty,
    NotFound,
    Request,
    SerDe,
    Search(search::SearcherError),
    Signature,
    TimelineQuery(timeline::query::QueryError),
    Unauthorized,
    Url,
    Webfinger,
    Expired,
    UserAlreadyExists,
    #[cfg(feature = "s3")]
    S3(s3::error::S3Error),
}

impl From<bcrypt::BcryptError> for Error {
    fn from(_: bcrypt::BcryptError) -> Self {
        Error::Signature
    }
}
pub const ITEMS_PER_PAGE: i32 = 12;
impl From<openssl::error::ErrorStack> for Error {
    fn from(_: openssl::error::ErrorStack) -> Self {
        Error::Signature
    }
}

impl From<sign::Error> for Error {
    fn from(_: sign::Error) -> Self {
        Error::Signature
    }
}

impl From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Self {
        Error::Db(err)
    }
}

impl From<url::ParseError> for Error {
    fn from(_: url::ParseError) -> Self {
        Error::Url
    }
}

impl From<iri_string::validate::Error> for Error {
    fn from(_: iri_string::validate::Error) -> Self {
        Error::Url
    }
}

impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Error::SerDe
    }
}

impl From<reqwest::Error> for Error {
    fn from(_: reqwest::Error) -> Self {
        Error::Request
    }
}

impl From<reqwest::header::InvalidHeaderValue> for Error {
    fn from(_: reqwest::header::InvalidHeaderValue) -> Self {
        Error::Request
    }
}

impl From<activitystreams::checked::CheckError> for Error {
    fn from(_: activitystreams::checked::CheckError) -> Error {
        Error::MissingApProperty
    }
}

impl From<webfinger::WebfingerError> for Error {
    fn from(_: webfinger::WebfingerError) -> Self {
        Error::Webfinger
    }
}

impl From<search::SearcherError> for Error {
    fn from(err: search::SearcherError) -> Self {
        Error::Search(err)
    }
}

impl From<timeline::query::QueryError> for Error {
    fn from(err: timeline::query::QueryError) -> Self {
        Error::TimelineQuery(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<InboxError<Error>> for Error {
    fn from(err: InboxError<Error>) -> Error {
        match err {
            InboxError::InvalidActor(Some(e)) | InboxError::InvalidObject(Some(e)) => e,
            e => Error::Inbox(Box::new(e)),
        }
    }
}

impl From<request::Error> for Error {
    fn from(_err: request::Error) -> Error {
        Error::Request
    }
}

#[cfg(feature = "s3")]
impl From<s3::error::S3Error> for Error {
    fn from(err: s3::error::S3Error) -> Error {
        Error::S3(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Adds a function to a model, that returns the first
/// matching row for a given list of fields.
///
/// Usage:
///
/// ```ignore
/// impl Model {
///     find_by!(model_table, name_of_the_function, field1 as String, field2 as i32);
/// }
///
/// // Get the Model with field1 == "", and field2 == 0
/// Model::name_of_the_function(connection, String::new(), 0);
/// ```
macro_rules! find_by {
    ($table:ident, $fn:ident, $($col:ident as $type:ty),+) => {
        /// Try to find a $table with a given $col
        pub fn $fn(conn: &crate::Connection, $($col: $type),+) -> Result<Self> {
            $table::table
                $(.filter($table::$col.eq($col)))+
                .first(conn)
                .map_err(Error::from)
        }
    };
}

/// List all rows of a model, with field-based filtering.
///
/// Usage:
///
/// ```ignore
/// impl Model {
///     list_by!(model_table, name_of_the_function, field1 as String);
/// }
///
/// // To get all Models with field1 == ""
/// Model::name_of_the_function(connection, String::new());
/// ```
macro_rules! list_by {
    ($table:ident, $fn:ident, $($col:ident as $type:ty),+) => {
        /// Try to find a $table with a given $col
        pub fn $fn(conn: &crate::Connection, $($col: $type),+) -> Result<Vec<Self>> {
            $table::table
                $(.filter($table::$col.eq($col)))+
                .load::<Self>(conn)
                .map_err(Error::from)
        }
    };
}

/// Adds a function to a model to retrieve a row by ID
///
/// # Usage
///
/// ```ignore
/// impl Model {
///     get!(model_table);
/// }
///
/// // Get the Model with ID 1
/// Model::get(connection, 1);
/// ```
macro_rules! get {
    ($table:ident) => {
        pub fn get(conn: &crate::Connection, id: i32) -> Result<Self> {
            $table::table
                .filter($table::id.eq(id))
                .first(conn)
                .map_err(Error::from)
        }
    };
}

/// Adds a function to a model to insert a new row
///
/// # Usage
///
/// ```ignore
/// impl Model {
///     insert!(model_table, NewModelType);
/// }
///
/// // Insert a new row
/// Model::insert(connection, NewModelType::new());
/// ```
macro_rules! insert {
    ($table:ident, $from:ty) => {
        insert!($table, $from, |x, _conn| Ok(x));
    };
    ($table:ident, $from:ty, |$val:ident, $conn:ident | $( $after:tt )+) => {
        last!($table);

        #[allow(dead_code)]
        pub fn insert(conn: &crate::Connection, new: $from) -> Result<Self> {
            diesel::insert_into($table::table)
                .values(new)
                .execute(conn)?;
            #[allow(unused_mut)]
            let mut $val = Self::last(conn)?;
            let $conn = conn;
            $( $after )+
        }
    };
}

/// Returns the last row of a table.
///
/// # Usage
///
/// ```ignore
/// impl Model {
///     last!(model_table);
/// }
///
/// // Get the last Model
/// Model::last(connection)
/// ```
macro_rules! last {
    ($table:ident) => {
        #[allow(dead_code)]
        pub fn last(conn: &crate::Connection) -> Result<Self> {
            $table::table
                .order_by($table::id.desc())
                .first(conn)
                .map_err(Error::from)
        }
    };
}

mod config;
pub use config::CONFIG;

pub fn ap_url(url: &str) -> String {
    format!("https://{}", url)
}

pub trait SmtpNewWithAddr {
    fn new_with_addr(
        addr: (&str, u16),
    ) -> std::result::Result<smtp::SmtpClient, smtp::error::Error>;
}

impl SmtpNewWithAddr for smtp::SmtpClient {
    // Stolen from lettre::smtp::SmtpClient::new_simple()
    fn new_with_addr(addr: (&str, u16)) -> std::result::Result<Self, smtp::error::Error> {
        use native_tls::TlsConnector;
        use smtp::{
            client::net::{ClientTlsParameters, DEFAULT_TLS_PROTOCOLS},
            ClientSecurity, SmtpClient,
        };

        let (domain, port) = addr;

        let mut tls_builder = TlsConnector::builder();
        tls_builder.min_protocol_version(Some(DEFAULT_TLS_PROTOCOLS[0]));

        let tls_parameters =
            ClientTlsParameters::new(domain.to_string(), tls_builder.build().unwrap());

        SmtpClient::new((domain, port), ClientSecurity::Wrapper(tls_parameters))
    }
}

#[cfg(test)]
#[macro_use]
mod tests {
    use crate::{db_conn, migrations::IMPORTED_MIGRATIONS, Connection as Conn, CONFIG};
    use chrono::{naive::NaiveDateTime, Datelike, Timelike};
    use diesel::r2d2::ConnectionManager;
    use plume_common::utils::random_hex;
    use std::env::temp_dir;

    #[macro_export]
    macro_rules! part_eq {
        ( $x:expr, $y:expr, [$( $var:ident ),*] ) => {
            {
                $(
                    assert_eq!($x.$var, $y.$var);
                )*
            }
        };
    }

    pub fn db() -> db_conn::DbConn {
        db_conn::DbConn((*DB_POOL).get().unwrap())
    }

    lazy_static! {
        static ref DB_POOL: db_conn::DbPool = {
            let pool = db_conn::DbPool::builder()
                .connection_customizer(Box::new(db_conn::tests::TestConnectionCustomizer))
                .build(ConnectionManager::<Conn>::new(CONFIG.database_url.as_str()))
                .unwrap();
            let dir = temp_dir().join(format!("plume-test-{}", random_hex()));
            IMPORTED_MIGRATIONS
                .run_pending_migrations(&pool.get().unwrap(), &dir)
                .expect("Migrations error");
            pool
        };
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn format_datetime(dt: &NaiveDateTime) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
            dt.timestamp_subsec_micros()
        )
    }

    #[cfg(feature = "sqlite")]
    pub(crate) fn format_datetime(dt: &NaiveDateTime) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        )
    }
}

pub mod admin;
pub mod api_tokens;
pub mod apps;
pub mod blocklisted_emails;
pub mod blog_authors;
pub mod blogs;
pub mod comment_seers;
pub mod comments;
pub mod db_conn;
pub mod email_signups;
pub mod follows;
pub mod headers;
pub mod inbox;
pub mod instance;
pub mod likes;
pub mod lists;
pub mod medias;
pub mod mentions;
pub mod migrations;
pub mod notifications;
pub mod password_reset_requests;
pub mod plume_rocket;
pub mod post_authors;
pub mod posts;
pub mod remote_fetch_actor;
pub mod reshares;
pub mod safe_string;
#[allow(unused_imports)]
pub mod schema;
pub mod search;
pub mod signups;
pub mod tags;
pub mod timeline;
pub mod users;
pub use plume_rocket::PlumeRocket;
