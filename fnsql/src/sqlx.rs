pub use sqlx;

/// Returns a connection pool to an in-memory SQLite database for testing.
pub async fn testing_pool() -> Result<sqlx::SqlitePool, sqlx::Error> {
    sqlx::SqlitePool::connect("sqlite::memory:").await
}
