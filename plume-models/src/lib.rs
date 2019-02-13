#![feature(try_trait)]

extern crate activitypub;
extern crate ammonia;
extern crate askama_escape;
extern crate bcrypt;
extern crate canapi;
extern crate chrono;
#[macro_use]
extern crate diesel;
extern crate guid_create;
extern crate heck;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate openssl;
extern crate plume_api;
extern crate plume_common;
extern crate reqwest;
extern crate rocket;
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

#[cfg(test)]
#[macro_use]
extern crate diesel_migrations;

use std::env;

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

pub type Result<T> = std::result::Result<T, Error>;

pub type ApiResult<T> = std::result::Result<T, canapi::Error>;

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
        last!($table);

        pub fn insert(conn: &crate::Connection, new: $from) -> Result<Self> {
            diesel::insert_into($table::table)
                .values(new)
                .execute(conn)?;
            Self::last(conn)
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

lazy_static! {
    pub static ref BASE_URL: String = env::var("BASE_URL").unwrap_or_else(|_| format!(
        "127.0.0.1:{}",
        env::var("ROCKET_PORT").unwrap_or_else(|_| String::from("8000"))
    ));
    pub static ref USE_HTTPS: bool = env::var("USE_HTTPS").map(|val| val == "1").unwrap_or(true);
}

#[cfg(not(test))]
static DB_NAME: &str = "plume";
#[cfg(test)]
static DB_NAME: &str = "plume_tests";

#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
lazy_static! {
    pub static ref DATABASE_URL: String =
        env::var("DATABASE_URL").unwrap_or_else(|_| format!("postgres://plume:plume@localhost/{}", DB_NAME));
}

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
lazy_static! {
    pub static ref DATABASE_URL: String =
        env::var("DATABASE_URL").unwrap_or_else(|_| format!("{}.sqlite", DB_NAME));
}

pub fn ap_url(url: &str) -> String {
    let scheme = if *USE_HTTPS { "https" } else { "http" };
    format!("{}://{}", scheme, url)
}

#[cfg(test)]
#[macro_use]
mod tests {
    use diesel::{dsl::sql_query, Connection, RunQueryDsl};
    use Connection as Conn;
    use DATABASE_URL;

    #[cfg(feature = "sqlite")]
    embed_migrations!("../migrations/sqlite");

    #[cfg(feature = "postgres")]
    embed_migrations!("../migrations/postgres");

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

    pub fn db() -> Conn {
        let conn =
            Conn::establish(&*DATABASE_URL.as_str()).expect("Couldn't connect to the database");
        embedded_migrations::run(&conn).expect("Couldn't run migrations");
        #[cfg(feature = "sqlite")]
        sql_query("PRAGMA foreign_keys = on;").execute(&conn).expect("PRAGMA foreign_keys fail");
        conn
    }
}

pub mod admin;
pub mod api_tokens;
pub mod apps;
pub mod blog_authors;
pub mod blogs;
pub mod comments;
pub mod comment_seers;
pub mod db_conn;
pub mod follows;
pub mod headers;
pub mod instance;
pub mod likes;
pub mod medias;
pub mod mentions;
pub mod notifications;
pub mod post_authors;
pub mod posts;
pub mod reshares;
pub mod safe_string;
pub mod search;
pub mod schema;
pub mod tags;
pub mod users;
