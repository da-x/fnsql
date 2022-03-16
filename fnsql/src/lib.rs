extern crate fnsql_macro;

// Re-export macro
pub use fnsql_macro::fnsql;

#[cfg(feature = "with-postgres")]
pub mod postgres;
