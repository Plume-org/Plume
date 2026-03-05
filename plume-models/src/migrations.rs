use crate::Connection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness};

// Embed the migrations from the migrations folder into the application
// This way, the program automatically migrates the database to the latest version
// https://docs.rs/diesel_migrations/*/diesel_migrations/macro.embed_migrations.html
#[cfg(feature = "sqlite")]
mod sqlite_migrations {
    pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!("../migrations/sqlite");
}

// Embed the migrations from the migrations folder into the application
// This way, the program automatically migrates the database to the latest version
// https://docs.rs/diesel_migrations/*/diesel_migrations/macro.embed_migrations.html
#[cfg(feature = "postgres")]
mod postgres_migrations {
    pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!("../migrations/postgres");
}

fn migrations() -> EmbeddedMigrations {
    #[cfg(feature = "postgres")]
    return postgres_migrations::MIGRATIONS;

    #[cfg(feature = "sqlite")]
    return sqlite_migrations::MIGRATIONS;
}

pub fn run_pending_migrations(conn: &mut Connection) -> diesel::migration::Result<usize> {
    conn.run_pending_migrations(migrations()).map(|v| v.len())
}

pub fn is_pending(conn: &mut Connection) -> diesel::migration::Result<bool> {
    conn.has_pending_migration(migrations())
}
