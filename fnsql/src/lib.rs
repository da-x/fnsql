extern crate fnsql_macro;

// Re-export macro
pub use fnsql_macro::fnsql;

#[cfg(feature = "with-postgres")]
pub mod postgres;

#[cfg(feature = "with-sqlx-sqlite")]
pub mod sqlx;
