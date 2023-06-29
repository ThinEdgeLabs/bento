use diesel::pg::PgConnection;
use diesel::r2d2;
use std::env;

pub type DbPool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;

pub fn initialize_db_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").expect("Missing DATABASE_URL");
    let manager = r2d2::ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool")
}
