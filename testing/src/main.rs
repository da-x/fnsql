extern crate postgres as crate_postgres;

mod sqlx_sqlite;
mod postgres;

#[tokio::main]
async fn main() {
    sqlx_sqlite::main().await.unwrap();
    postgres::main().unwrap();
}
