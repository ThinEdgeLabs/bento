use diesel::pg::PgConnection;
use diesel::r2d2;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::env;
use std::error::Error;

pub type DbPool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");
pub type DbError = Box<dyn Error + Send + Sync + 'static>;

pub fn initialize_db_pool() -> DbPool {
    let postgres_user = env::var("POSTGRES_USER").expect("Missing POSTGRES_USER");
    let postgres_password = env::var("POSTGRES_PASSWORD").expect("Missing POSTGRES_PASSWORD");
    let postgres_host = env::var("POSTGRES_HOST").expect("Missing POSTGRES_HOST");
    let postgres_db = env::var("POSTGRES_DB").expect("Missing POSTGRES_DB");
    let database_url = format!(
        "postgres://{}:{}@{}/{}",
        postgres_user, postgres_password, postgres_host, postgres_db
    );
    let manager = r2d2::ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool")
}

pub fn run_migrations(
    connection: &mut impl MigrationHarness<diesel::pg::Pg>,
) -> Result<(), DbError> {
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
    Ok(())
}
