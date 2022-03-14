extern crate postgres as crate_postgres;

mod sqlite;
mod postgres;

fn main() {
    sqlite::main().unwrap();
    postgres::main().unwrap();
}
