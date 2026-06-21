//! Runtime support for the `sqlx_sqlite` backend.
//!
//! This module is available when the `with-sqlx-sqlite` feature is enabled.
//! It re-exports `sqlx` for convenience and provides a testing helper.
//!
//! # Testing
//!
//! Use [`testing_pool`] to get an in-memory SQLite pool for tests:
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), sqlx::Error> {
//! let pool = fnsql::sqlx::testing_pool().await?;
//! // use pool...
//! # Ok(()) }
//! ```

pub use sqlx;

/// Returns a connection pool to an in-memory SQLite database for testing.
///
/// Each call creates a new, isolated in-memory database.
pub async fn testing_pool() -> Result<sqlx::SqlitePool, sqlx::Error> {
    sqlx::SqlitePool::connect("sqlite::memory:").await
}
