#![feature(try_trait)]
#![feature(never_type)]
#![feature(custom_attribute)]
#![feature(proc_macro_hygiene)]

extern crate activitypub;
extern crate ammonia;
extern crate askama_escape;
extern crate bcrypt;
extern crate chrono;
#[macro_use]
extern crate diesel;
extern crate guid_create;
extern crate heck;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate ldap3;
extern crate migrations_internals;
extern crate openssl;
extern crate plume_api;
extern crate plume_common;
#[macro_use]
extern crate plume_macro;
extern crate reqwest;
extern crate rocket;
extern crate rocket_i18n;
extern crate scheduled_thread_pool;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate tantivy;
extern crate url;
extern crate webfinger;
extern crate whatlang;

use plume_common::activity_pub::inbox::InboxError;

#[cfg(not(any(feature = "sqlite", feature = "postgres")))]
compile_error!("Either feature \"sqlite\" or \"postgres\" must be enabled for this crate.");
#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("Either feature \"sqlite\" or \"postgres\" must be enabled for this crate.");

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
pub type Connection = diesel::SqliteConnection;

#[cfg(all(not(feature = "sqlite"), feature = "postgres"))]
pub type Connection = diesel::PgConnection;

/// All the possible errors that can be encoutered in this crate
#[derive(Debug)]
pub enum Error {
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
    Unauthorized,
    Url,
    Webfinger,
    Expired,
}

impl From<bcrypt::BcryptError> for Error {
    fn from(_: bcrypt::BcryptError) -> Self {
        Error::Signature
    }
}

impl From<openssl::error::ErrorStack> for Error {
    fn from(_: openssl::error::ErrorStack) -> Self {
        Error::Signature
    }
}

impl From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Self {
        Error::Db(err)
    }
}

impl From<std::option::NoneError> for Error {
    fn from(_: std::option::NoneError) -> Self {
        Error::NotFound
    }
}

impl From<url::ParseError> for Error {
    fn from(_: url::ParseError) -> Self {
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

impl From<activitypub::Error> for Error {
    fn from(err: activitypub::Error) -> Self {
        match err {
            activitypub::Error::NotFound => Error::MissingApProperty,
            _ => Error::SerDe,
        }
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

pub type Result<T> = std::result::Result<T, Error>;

/// Adds a function to a model, that returns the first
/// matching row for a given list of fields.
///
/// Usage:
///
/// ```rust
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
                .limit(1)
                .load::<Self>(conn)?
                .into_iter()
                .next()
                .ok_or(Error::NotFound)
        }
    };
}

/// List all rows of a model, with field-based filtering.
///
/// Usage:
///
/// ```rust
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
/// ```rust
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
                .limit(1)
                .load::<Self>(conn)?
                .into_iter()
                .next()
                .ok_or(Error::NotFound)
        }
    };
}

/// Adds a function to a model to insert a new row
///
/// # Usage
///
/// ```rust
/// impl Model {
///     insert!(model_table, NewModelType);
/// }
///
/// // Insert a new row
/// Model::insert(connection, NewModelType::new());
/// ```
macro_rules! insert {
    ($table:ident, $from:ident) => {
        insert!($table, $from, |x, _conn| Ok(x));
    };
    ($table:ident, $from:ident, |$val:ident, $conn:ident | $( $after:tt )+) => {
        last!($table);

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
/// ```rust
/// impl Model {
///     last!(model_table);
/// }
///
/// // Get the last Model
/// Model::last(connection)
/// ```
macro_rules! last {
    ($table:ident) => {
        pub fn last(conn: &crate::Connection) -> Result<Self> {
            $table::table
                .order_by($table::id.desc())
                .limit(1)
                .load::<Self>(conn)?
                .into_iter()
                .next()
                .ok_or(Error::NotFound)
        }
    };
}

mod config;
pub use config::CONFIG;

pub fn ap_url(url: &str) -> String {
    format!("https://{}", url)
}

#[cfg(test)]
#[macro_use]
mod tests {
    use db_conn;
    use diesel::r2d2::ConnectionManager;
    #[cfg(feature = "sqlite")]
    use diesel::{dsl::sql_query, RunQueryDsl};
    use migrations::IMPORTED_MIGRATIONS;
    use plume_common::utils::random_hex;
    use scheduled_thread_pool::ScheduledThreadPool;
    use search;
    use std::env::temp_dir;
    use std::sync::Arc;
    use Connection as Conn;
    use CONFIG;

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

    pub fn db<'a>() -> db_conn::DbConn {
        db_conn::DbConn((*DB_POOL).get().unwrap())
    }

    lazy_static! {
        static ref DB_POOL: db_conn::DbPool = {
            let pool = db_conn::DbPool::builder()
                .connection_customizer(Box::new(db_conn::PragmaForeignKey))
                .build(ConnectionManager::<Conn>::new(CONFIG.database_url.as_str()))
                .unwrap();
            let dir = temp_dir().join(format!("plume-test-{}", random_hex()));
            IMPORTED_MIGRATIONS
                .run_pending_migrations(&pool.get().unwrap(), &dir)
                .expect("Migrations error");
            pool
        };
    }

    pub fn rockets() -> super::PlumeRocket {
        super::PlumeRocket {
            conn: db_conn::DbConn((*DB_POOL).get().unwrap()),
            searcher: Arc::new(search::tests::get_searcher()),
            worker: Arc::new(ScheduledThreadPool::new(2)),
            user: None,
        }
    }
}

pub mod admin;
pub mod api_tokens;
pub mod apps;
pub mod blog_authors;
pub mod blogs;
pub mod comment_seers;
pub mod comments;
pub mod db_conn;
pub mod follows;
pub mod headers;
pub mod inbox;
pub mod instance;
pub mod ldap;
pub mod likes;
pub mod medias;
pub mod mentions;
pub mod migrations;
pub mod notifications;
pub mod password_reset_requests;
pub mod plume_rocket;
pub mod post_authors;
pub mod posts;
pub mod reshares;
pub mod safe_string;
pub mod schema;
pub mod search;
pub mod tags;
pub mod users;
pub use plume_rocket::PlumeRocket;
