The `fnsql` crate provides simple type-safe wrappers around SQL queries.
Instead of calling type-less `.query()` and `.execute()`, you call auto-generated
unique wrappers that are strongly typed: `.query_<name>()` and `.execute_<name>()`.
You manually specify the input and output types, but only once, with the query,
in separation from the code that uses the query.

It's a very simple implementation that doesn't force any schema or ORM down
your throat, so if you are already using the `sqlx` or `postgres` crates,
you can gradually replace your type-less queries with the type-ful wrappers,
or migrate from an opinionated ORM.

## Quick start (sqlx_sqlite)

```rust,no_run
fnsql::fnsql! {
    #[sqlx_sqlite, test]
    create_table_pet() {
        "CREATE TABLE pet (
              id      INTEGER PRIMARY KEY,
              name    TEXT NOT NULL,
              data    BLOB
        )"
    }

    #[sqlx_sqlite, test(with=[create_table_pet])]
    insert_new_pet(name: String, data: Option<Vec<u8>>) {
        "INSERT INTO pet (name, data) VALUES (:name, :data)"
    }

    #[sqlx_sqlite, test(with=[create_table_pet])]
    get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>)] {
        "SELECT id, data FROM pet WHERE pet.name = :name"
    }
}
```

The generated methods are extension methods on `sqlx::SqlitePool`:

```rust,no_run
# async fn example() -> Result<(), sqlx::Error> {
let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;

// DDL/DML — returns rows affected
pool.execute_create_table_pet().await?;
pool.execute_insert_new_pet(&"Max".to_string(), &None).await?;

// Query returning multiple rows as Vec of typed tuples
let rows = pool.query_get_pet_id_data(&Some("Max".to_string())).await?;
for (id, data) in rows {
    println!("Found pet id={:?}, data={:?}", id, data);
}

// Query returning exactly one row
let (id, data) = pool.query_one_get_pet_id_data(&Some("Max".to_string())).await?;

// Query that may return zero or one row
let row = pool.query_opt_get_pet_id_data(&Some("Nonexistent".to_string())).await?;
# Ok(()) }
```

## Quick start (postgres)

For postgres, named parameters are transformed to positional `$1`, `$2`, etc.
when the `named` attribute is used:

```rust,no_run
fnsql::fnsql! {
    #[postgres]
    create_table_pet() {
        "CREATE TABLE pet (id SERIAL PRIMARY KEY, name TEXT NOT NULL)"
    }

    #[postgres, named]
    insert_new_pet(id: i32, name: String) {
        "INSERT INTO pet (id, name) VALUES (:id, :name)"
    }
}
```

Generated methods are extension methods on `postgres::Client` and
`postgres::Transaction<'a>`, with both direct and prepared variants:

```rust,no_run
# fn example(conn: &mut postgres::Client) -> Result<(), postgres::Error> {
conn.execute_create_table_pet()?;
conn.execute_insert_new_pet(&1, &"Max".to_string())?;

// Prepared statement variant
let prep = conn.prepare_insert_new_pet()?;
conn.execute_prepared_insert_new_pet(&prep, &2, &"Rex".to_string())?;
# Ok(()) }
```

## Generated API

### sqlx_sqlite (`sqlx::SqlitePool`)

For each query `<name>(p1: T1, p2: T2) -> [(O1, O2)]`, the following async
methods are generated on `sqlx::SqlitePool`:

| Method | Return Type | Description |
|--------|------------|-------------|
| `execute_<name>(&self, &p1, &p2)` | `Result<u64, sqlx::Error>` | Execute, returns rows affected |
| `query_<name>(&self, &p1, &p2)` | `Result<Vec<(O1, O2)>, sqlx::Error>` | Fetch all matching rows |
| `query_one_<name>(&self, &p1, &p2)` | `Result<(O1, O2), sqlx::Error>` | Fetch exactly one row |
| `query_opt_<name>(&self, &p1, &p2)` | `Result<Option<(O1, O2)>, sqlx::Error>` | Fetch zero or one row |

Named parameters in the SQL (`:name`) are automatically transformed to
positional `$N` placeholders at compile time.

A `convert_row_<name>(row: SqliteRow) -> Result<(O1, O2), sqlx::Error>`
function is also generated for manual row conversion.

### postgres (`postgres::Client` / `postgres::Transaction<'a>`)

For each query `<name>(p1: T1, p2: T2) -> [(O1, O2)]`, the following methods
are generated on both `postgres::Client` and `postgres::Transaction<'a>`:

| Method | Return Type | Description |
|--------|------------|-------------|
| `execute_<name>(&mut self, &p1, &p2)` | `Result<u64, postgres::Error>` | Execute directly |
| `prepare_<name>(&mut self)` | `Result<<name>Statement_, postgres::Error>` | Prepare statement |
| `execute_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<u64, postgres::Error>` | Execute prepared |
| `query_<name>(&mut self, &p1, &p2)` | `Result<Vec<(O1, O2)>, postgres::Error>` | Fetch all rows |
| `query_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<Vec<(O1, O2)>, postgres::Error>` | Fetch all, prepared |
| `query_one_<name>(&mut self, &p1, &p2)` | `Result<(O1, O2), postgres::Error>` | Fetch one row |
| `query_one_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<(O1, O2), postgres::Error>` | Fetch one, prepared |
| `query_opt_<name>(&mut self, &p1, &p2)` | `Result<Option<(O1, O2)>, postgres::Error>` | Fetch opt row |
| `query_opt_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<Option<(O1, O2)>, postgres::Error>` | Fetch opt, prepared |

With the `prepare-cache` feature enabled, an additional `prepare_cached_<name>()`
method is available that uses `fnsql::postgres::Cache`.

## Attributes

Each query declaration starts with attributes in square brackets:

```text
#[<backend>, <attr2>, <attr3>, ...]
```

**Backend (required, exactly one):**
- `sqlx_sqlite` — generates async methods on `sqlx::SqlitePool`
- `postgres` — generates sync methods on `postgres::Client` / `Transaction`

**Optional attributes:**
- `test` — generates an auto-test that runs the query with arbitrary values
- `test(with=[other_query])` — same as `test`, but runs prerequisite queries first
- `named` (postgres only) — transforms `:name` placeholders to `$1`, `$2`, etc.
- `conststr=<NAME>` — generates a `pub const NAME: &str` with the query string

## Parameters and return types

**Parameters** use Rust-like syntax: `param_name: Type`. The generated methods
accept references: `&param_name`. Supported types are anything that implements
the backend's respective trait (`sqlx::Encode` for sqlx, `postgres::types::ToSql`
for postgres).

Common type shortcuts:
- `str` is accepted as a parameter type (maps to `&str`)
- `[u8]` is accepted as a parameter type (maps to `&[u8]`)

**Return types** are optional and specified as `-> [(T1, T2, ...)]`. Each type
corresponds to a column in the result set. If omitted, the query is treated as
returning no data (DDL/DML).

## Auto-generated tests

With the `test` attribute, fnsql generates a `#[test]` (or `#[tokio::test]` for
sqlx_sqlite) that opens an in-memory database, runs prerequisite queries via
`test(with=[...])`, and executes the query with arbitrary values. This validates
that your query syntax is correct without writing any test code.

To enable test compilation, add to your `[dev-dependencies]`:

```toml
arbitrary = { version = "1", features = ["derive"] }
```