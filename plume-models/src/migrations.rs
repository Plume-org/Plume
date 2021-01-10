use crate::{Connection, Error, Result};
use diesel::connection::{Connection as Conn, SimpleConnection};
use migrations_internals::{setup_database, MigrationConnection};
use std::path::Path;
use tracing::info;

#[allow(dead_code)] //variants might not be constructed if not required by current migrations
enum Action {
    Sql(&'static str),
    Function(&'static dyn Fn(&Connection, &Path) -> Result<()>),
}

impl Action {
    fn run(&self, conn: &Connection, path: &Path) -> Result<()> {
        match self {
            Action::Sql(sql) => conn.batch_execute(sql).map_err(Error::from),
            Action::Function(f) => f(conn, path),
        }
    }
}

struct ComplexMigration {
    name: &'static str,
    up: &'static [Action],
    down: &'static [Action],
}

impl ComplexMigration {
    fn run(&self, conn: &Connection, path: &Path) -> Result<()> {
        info!("Running migration {}", self.name);
        for step in self.up {
            step.run(conn, path)?
        }
        Ok(())
    }

    fn revert(&self, conn: &Connection, path: &Path) -> Result<()> {
        info!("Reverting migration {}", self.name);
        for step in self.down {
            step.run(conn, path)?
        }
        Ok(())
    }
}

pub struct ImportedMigrations(&'static [ComplexMigration]);

impl ImportedMigrations {
    pub fn run_pending_migrations(&self, conn: &Connection, path: &Path) -> Result<()> {
        use diesel::dsl::sql;
        use diesel::sql_types::Bool;
        use diesel::{select, RunQueryDsl};
        #[cfg(feature = "postgres")]
        let schema_exists: bool = select(sql::<Bool>(
            "EXISTS \
             (SELECT 1 \
             FROM information_schema.tables \
             WHERE table_name = '__diesel_schema_migrations')",
        ))
        .get_result(conn)?;
        #[cfg(feature = "sqlite")]
        let schema_exists: bool = select(sql::<Bool>(
            "EXISTS \
             (SELECT 1 \
             FROM sqlite_master \
             WHERE type = 'table' \
             AND name = '__diesel_schema_migrations')",
        ))
        .get_result(conn)?;

        if !schema_exists {
            setup_database(conn)?;
        }

        let latest_migration = conn.latest_run_migration_version()?;
        let latest_id = if let Some(migration) = latest_migration {
            self.0
                .binary_search_by_key(&migration.as_str(), |mig| mig.name)
                .map(|id| id + 1)
                .map_err(|_| Error::NotFound)?
        } else {
            0
        };

        let to_run = &self.0[latest_id..];
        for migration in to_run {
            conn.transaction(|| {
                migration.run(conn, path)?;
                conn.insert_new_migration(migration.name)
                    .map_err(Error::from)
            })?;
        }
        Ok(())
    }

    pub fn is_pending(&self, conn: &Connection) -> Result<bool> {
        let latest_migration = conn.latest_run_migration_version()?;
        if let Some(migration) = latest_migration {
            Ok(self.0.last().expect("no migrations found").name != migration)
        } else {
            Ok(true)
        }
    }

    pub fn rerun_last_migration(&self, conn: &Connection, path: &Path) -> Result<()> {
        let latest_migration = conn.latest_run_migration_version()?;
        let id = latest_migration
            .and_then(|m| self.0.binary_search_by_key(&m.as_str(), |m| m.name).ok())?;
        let migration = &self.0[id];
        conn.transaction(|| {
            migration.revert(conn, path)?;
            migration.run(conn, path)
        })
    }
}

pub const IMPORTED_MIGRATIONS: ImportedMigrations = {
    import_migrations! {}
};
