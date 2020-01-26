use crate::Connection;
use diesel::r2d2::{
    ConnectionManager, CustomizeConnection, Error as ConnError, Pool, PooledConnection,
};
#[cfg(feature = "sqlite")]
use diesel::{dsl::sql_query, ConnectionError, RunQueryDsl};
use rocket::{
    http::Status,
    request::{self, FromRequestAsync},
    Outcome, Request,
};
use std::ops::Deref;

pub type DbPool = Pool<ConnectionManager<Connection>>;

// From rocket documentation

// Connection request guard type: a wrapper around an r2d2 pooled connection.
pub struct DbConn(pub PooledConnection<ConnectionManager<Connection>>);

/// Attempts to retrieve a single connection from the managed database pool. If
/// no pool is currently managed, fails with an `InternalServerError` status. If
/// no connections are available, fails with a `ServiceUnavailable` status.
impl<'a, 'r> FromRequestAsync<'a, 'r> for DbConn {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::FromRequestFuture<'a, Self, Self::Error> {
        Box::pin(async move {
            match DbConn::from_request(request).await {
                Outcome::Success(a) => return Outcome::Success(a),
                _ => return Outcome::Failure((Status::ServiceUnavailable, ())),
            };
        })
    }
}

// For the convenience of using an &DbConn as an &Connection.
impl Deref for DbConn {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Execute a pragma for every new sqlite connection
#[derive(Debug)]
pub struct PragmaForeignKey;
impl CustomizeConnection<Connection, ConnError> for PragmaForeignKey {
    #[cfg(feature = "sqlite")] // will default to an empty function for postgres
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), ConnError> {
        sql_query("PRAGMA foreign_keys = on;")
            .execute(conn)
            .map(|_| ())
            .map_err(|_| {
                ConnError::ConnectionError(ConnectionError::BadConnection(String::from(
                    "PRAGMA foreign_keys = on failed",
                )))
            })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use diesel::Connection as _;

    #[derive(Debug)]
    pub struct TestConnectionCustomizer;
    impl CustomizeConnection<Connection, ConnError> for TestConnectionCustomizer {
        fn on_acquire(&self, conn: &mut Connection) -> Result<(), ConnError> {
            PragmaForeignKey.on_acquire(conn)?;
            Ok(conn.begin_test_transaction().unwrap())
        }
    }
}
