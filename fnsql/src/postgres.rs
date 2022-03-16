//! Support for PostgreSQL with fnsql.
//!
//! **Dependent on the `with-postgres` manifest feature**.
//!
//! By default arguments subsitution uses $1 and $2 instead of ':name', unless
//! you provide the attribute 'named'.
//!
//!
//! ## Cache for prepared statement
//!
//! **Dependent on the `prepare-cache` manifest feature**.
//!
//! The fnsql 'prepare' method in fnsql for PostgreSQL returns a uniuqe type
//! per query. Behind the scene, there's also a `prepare_cached` that can
//! cache the prepared statements, but it needs to be passed the `Cache`
//! object.
//!
//!
//! ## Auto-genreated tests
//!
//! For the auto-generated tests to work, some PostgreSQL server needs to be
//! available for connectivity. All schemas are done with `pg_temp`, so no
//! actual tables are created.
//!
//! Use the following macro somewhere in your crate:
//!
//! ```ignore
//!     fnsql::fnsql_define_postgres_test_handlers!(docker_up, docker_down)
//! ```
//!
//! This generates two special ignored tests for bring-up/tear-down
//! docker-compose based PostgreSQL setup, to be added to a testing
//! environment as such:
//!
//! ```sh ignore
//! export FNSQL_TEST_POSTGRES_PORT=5433
//! cargo test -- tests::docker_up --ignored
//! sleep 2;
//! cargo test
//! cargo test -- tests::docker_down --ignored
//! ```
//!
//! This is needed as Rust does not provide provisions for test environment
//! bring-up/tear-down that are external to the process. Doing so on every
//! test would have been quite expensive in run-time.

#[cfg(feature = "prepare-cache")]
pub mod cache;
#[cfg(feature = "prepare-cache")]
pub use cache::Cache;
use postgres::{NoTls, Client};
pub use postgres::Error;

use std::io::{Write};
use std::fs::File;
use std::path::PathBuf;

static SQL_SETUP: &'static str = include_str!("postgres/sql_setup.sh");
static DOCKER_COMPOSE: &'static str = include_str!("postgres/docker-compose.yml");

pub fn get_docker_compose() -> String {
    let mut s = String::from(DOCKER_COMPOSE);
    let port = std::env::var("FNSQL_TEST_POSTGRES_PORT")
        .expect("undefined FNSQL_TEST_POSTGRES_PORT");
    s = s.replace("${FNSQL_TEST_POSTGRES_PORT}", &port);

    s
}

pub fn with_docker_compose<F>(f: F) -> Result<(), std::io::Error>
    where F: FnOnce(PathBuf) -> Result<(), std::io::Error>
{
    let tmp_dir = tempdir::TempDir::new("fnsql-postgres-docker")?;

    let file_path = tmp_dir.path().join("docker-compose.yaml");
    let mut tmp_file = File::create(&file_path)?;
    writeln!(tmp_file, "{}", get_docker_compose())?;
    drop(tmp_file);

    let sql_setup = tmp_dir.path().join("sql_setup.sh");
    let mut tmp_file = File::create(&sql_setup)?;
    writeln!(tmp_file, "{}", SQL_SETUP)?;
    drop(tmp_file);

    let r = f(file_path);

    tmp_dir.close()?;

    r
}

pub fn testing_client() -> Result<postgres::Client, postgres::Error> {
    let port = std::env::var("FNSQL_TEST_POSTGRES_PORT")
        .expect("undefined FNSQL_TEST_POSTGRES_PORT");
    let settings = format!("user=postgres host=localhost port={}", port);
    let client = Client::connect(&settings, NoTls)?;
    Ok(client)
}

pub fn testing_docker_up() -> Result<(), std::io::Error> {
    with_docker_compose(|path| {
        std::process::Command::new("docker-compose")
            .arg("-p").arg(module_path!())
            .arg("-f").arg(path)
            .arg("up").arg("-d").output()?;
        Ok(())
    })
}

pub fn testing_docker_down() -> Result<(), std::io::Error> {
    with_docker_compose(|path| {
        std::process::Command::new("docker-compose")
            .arg("-p").arg(module_path!())
            .arg("-f").arg(path)
            .arg("down").output()?;
        Ok(())
    })
}

#[macro_export]
macro_rules! fnsql_define_postgres_test_handlers {
    ($name_up:ident, $name_down:ident) => {
        #[cfg(test)]
        mod tests {
            #[ignore]
            #[test]
            fn $name_up() -> Result<(), std::io::Error> {
                $crate::postgres::testing_docker_up()
            }

            #[ignore]
            #[test]
            fn $name_down() -> Result<(), std::io::Error> {
                $crate::postgres::testing_docker_down()
            }
        }
    };
}

fnsql_define_postgres_test_handlers!(docker_up, docker_down);
