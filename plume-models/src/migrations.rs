use Connection;
use Error;
use Result;

use diesel::connection::{Connection as Conn, SimpleConnection};
use migrations_internals::MigrationConnection;

use std::path::Path;

#[allow(dead_code)] //variants might not be constructed if not required by current migrations
enum Action {
    Sql(&'static str),
    Function(&'static Fn(&Connection, &Path) -> Result<()>),
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
        for step in self.up {
            step.run(conn, path)?
        }
        Ok(())
    }

    fn revert(&self, conn: &Connection, path: &Path) -> Result<()> {
        for step in self.down {
            step.run(conn, path)?
        }
        Ok(())
    }
}

pub struct ImportedMigrations(&'static [ComplexMigration]);

impl ImportedMigrations {
    pub fn run_pending_migrations(&self, conn: &Connection, path: &Path) -> Result<()> {
        let latest_migration = conn.latest_run_migration_version()?;
        let latest_id = if let Some(migration) = latest_migration {
            self.0
                .binary_search_by_key(&migration.as_str(), |mig| mig.name)
                .map_err(|_| Error::NotFound)?
        } else {
            0
        };

        let to_run = &self.0[latest_id..];
        for migration in to_run {
            conn.transaction(|| {
                conn.insert_new_migration(migration.name)?;
                migration.run(conn, path)
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

pub const IMPORTED_MIGRATION: ImportedMigrations = {
    import_migrations! {}
};
