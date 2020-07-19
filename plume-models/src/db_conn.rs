use crate::Connection;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Error as ConnError, Pool};
#[cfg(feature = "sqlite")]
use diesel::{dsl::sql_query, ConnectionError, RunQueryDsl};
use rocket_contrib::databases::diesel;

pub type DbPool = Pool<ConnectionManager<Connection>>;

// From rocket documentation

// Connection request guard type: a wrapper around an r2d2 pooled connection.
#[database("plume")]
pub struct DbConn(pub Connection);

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
