#![allow(proc_macro_derive_resolution_fallback)] // This can be removed after diesel-1.4
#![feature(crate_in_paths)]

extern crate activitypub;
extern crate ammonia;
extern crate bcrypt;
extern crate canapi;
extern crate chrono;
#[macro_use]
extern crate diesel;
extern crate guid_create;
extern crate heck;
#[macro_use]
extern crate lazy_static;
extern crate openssl;
extern crate plume_api;
extern crate plume_common;
extern crate reqwest;
extern crate rocket;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate url;
extern crate webfinger;

#[cfg(test)]
#[macro_use] extern crate diesel_migrations;

use std::env;

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
pub type Connection = diesel::SqliteConnection;

#[cfg(all(not(feature = "sqlite"), feature = "postgres"))]
pub type Connection = diesel::PgConnection;

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
    ($table:ident, $fn:ident, $($col:ident as $type:ident),+) => {
        /// Try to find a $table with a given $col
        pub fn $fn(conn: &crate::Connection, $($col: $type),+) -> Option<Self> {
            $table::table
                $(.filter($table::$col.eq($col)))+
                .limit(1)
                .load::<Self>(conn)
                .expect("macro::find_by: Error loading $table by $col")
                .into_iter().nth(0)
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
    ($table:ident, $fn:ident, $($col:ident as $type:ident),+) => {
        /// Try to find a $table with a given $col
        pub fn $fn(conn: &crate::Connection, $($col: $type),+) -> Vec<Self> {
            $table::table
                $(.filter($table::$col.eq($col)))+
                .load::<Self>(conn)
                .expect("macro::list_by: Error loading $table by $col")
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
        pub fn get(conn: &crate::Connection, id: i32) -> Option<Self> {
            $table::table.filter($table::id.eq(id))
                .limit(1)
                .load::<Self>(conn)
                .expect("macro::get: Error loading $table by id")
                .into_iter().nth(0)
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

        pub fn insert(conn: &crate::Connection, new: $from) -> Self {
            diesel::insert_into($table::table)
                .values(new)
                .execute(conn)
                .expect("macro::insert: Error saving new $table");
            Self::last(conn)
        }
    };
}

/// Adds a function to a model to save changes to a model.
/// The model should derive diesel::AsChangeset.
///
/// # Usage
///
/// ```rust
/// impl Model {
///     update!(model_table);
/// }
///
/// // Update and save changes
/// let m = Model::get(connection, 1);
/// m.foo = 42;
/// m.update(connection);
/// ```
macro_rules! update {
    ($table:ident) => {
        pub fn update(&self, conn: &crate::Connection) -> Self {
            diesel::update(self)
                .set(self)
                .execute(conn)
                .expect(concat!("macro::update: Error updating ", stringify!($table)));
            Self::get(conn, self.id)
                .expect(concat!("macro::update: ", stringify!($table), " we just updated doesn't exist anymore???"))
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
        pub fn last(conn: &crate::Connection) -> Self {
            $table::table.order_by($table::id.desc())
                .limit(1)
                .load::<Self>(conn)
                .expect(concat!("macro::last: Error getting last ", stringify!($table)))
                .iter().next()
                .expect(concat!("macro::last: No last ", stringify!($table)))
                .clone()
        }
    };
}

lazy_static! {
    pub static ref BASE_URL: String = env::var("BASE_URL")
        .unwrap_or(format!("127.0.0.1:{}", env::var("ROCKET_PORT").unwrap_or(String::from("8000"))));

    pub static ref USE_HTTPS: bool = env::var("USE_HTTPS").map(|val| val == "1").unwrap_or(true);
}

#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
lazy_static! {
    pub static ref DATABASE_URL: String = env::var("DATABASE_URL").unwrap_or(String::from("postgres://plume:plume@localhost/plume"));
}

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
lazy_static! {
    pub static ref DATABASE_URL: String = env::var("DATABASE_URL").unwrap_or(String::from("plume.sqlite"));
}

pub fn ap_url(url: String) -> String {
    let scheme = if *USE_HTTPS {
        "https"
    } else {
        "http"
    };
    format!("{}://{}", scheme, url)
}

#[cfg(test)]
#[macro_use]
mod tests {
    use diesel::Connection;
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
        let conn = Conn::establish(&*DATABASE_URL.as_str()).expect("Couldn't connect to the database");
        embedded_migrations::run(&conn).expect("Couldn't run migrations");
        conn
    }
}

pub mod admin;
pub mod api_tokens;
pub mod apps;
pub mod blog_authors;
pub mod blogs;
pub mod comments;
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
pub mod schema;
pub mod tags;
pub mod users;
