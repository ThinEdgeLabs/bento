use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::env;
use std::error::Error;

pub type DbPool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn initialize_db_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").expect("Missing DATABASE_URL");
    let manager = r2d2::ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool")
}

pub fn migration_connection() -> diesel::PgConnection {
    let database_url = env::var("DATABASE_URL").expect("Missing DATABASE_URL");
    let mut conn = diesel::PgConnection::establish(&database_url).unwrap();
    conn.begin_test_transaction().unwrap();
    conn
}

pub fn run_migrations(
    connection: &mut impl MigrationHarness<diesel::pg::Pg>,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    connection.run_pending_migrations(MIGRATIONS)?;
    Ok(())
}
