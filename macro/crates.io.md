The `fnsql` crate provides simple type-safe optional wrappers around SQL
queries. Instead of calling type-less `.query()` and `.execute()`, you call to
auto-generated unique wrappers that are strongly typed, `.query_<name>()` and
`.execute_<name>()`. However, you manually specify the input and output types,
but only once, with the query, and in separation with the code that uses the
query.

It's a very simple implementation that doesn't force any schema or ORM down
your throat, so if you are already using `rusqlite`, you can't gradually
replace your type-less queries with the type-ful wrappers.

The way to generate these wrappers is to specify input and output types for
each of the queries, for example, consider the following definitions
specified with `fnsql`, based on the `rusqlite` example:

```rust
fnsql::fnsql! {
    #[rusqlite, test]
    create_table_pet() {
        "CREATE TABLE pet (
              id      INTEGER PRIMARY KEY,
              name    TEXT NOT NULL,
              data    BLOB
        )"
    }

    #[rusqlite, test(with=[create_table_pet])]
    get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>)] {
        "SELECT id, data FROM pet WHERE pet.name = :name"
    }

    #[rusqlite, test(with=[create_table_pet])]
    insert_new_pet(name: String, data: Option<Vec<u8>>) {
        "INSERT INTO pet (name, data) VALUES (:name, :data)"
    }
}
```

The definitions can be used as such (commented out is how the previous
type-less interfaces were used):

```rust ignore
let mut conn = rusqlite::Connection::open_in_memory()?;

conn.execute_create_table_pet()?;
// conn.execute(
//    "CREATE TABLE pet (
//               id              INTEGER PRIMARY KEY,
//               name            TEXT NOT NULL,
//               data            BLOB
//               )",
//     [],
// )?;

conn.execute_insert_new_pet(&me.name, &me.data)?;
// conn.execute(
//     "INSERT INTO pet (name, data) VALUES (?1, ?2)",
//     params![me.name, me.data],
// )?;

let mut stmt = conn.prepare_get_pet_id_data()?;
// let mut stmt = conn.prepare("SELECT id, name, data FROM pet")?;

let pet_iter = stmt.query(&Some("Max".to_string()), |_id, data| {
    Ok::<_, rusqlite::Error>(Pet {
        _id,
        data,
        name: "Max".to_string(),
    })
})?;
// let pet_iter = stmt.query_map([], |row| {
//     Ok(Person {
//         id: row.get(0)?,
//         name: row.get(1)?,
//         data: row.get(2)?,
//     })
// })?;
```

## Technical discussion

The idea with this crate is to allow direct SQL usage but never use inline
queries or have type inference at the call-site. Instead, we declare each query
on top-level, giving each a name and designated accessor methods that derive
from the name.

- The types of named variables are give in a Rust-like syntax.
- The type of the returned row is also provided.
- `fnsql` does not make an assurances to make sure the types match the query,
  you will discover it with `cargo test` and no additional code.
- `fnsql` writes the tests for each of the queries.  - `Arbitrary` is used to
  generate parameter values.
- If testing one query depend on another, you can specify that with `test(with=[..])`.

```text
running 3 tests
test auto_create_table_pet ... ok
test auto_insert_new_pet ... ok
test auto_get_pet_id_data ... ok
```

The following is for allowing generated query tests to compile:

```toml
[dev-dependencies]
arbitrary = { version = "1", features = ["derive"] }
```

## Limitations

 * Though it <i>does</i> provide auto-generated tests for validating queries in `cargo test`,
   it does not do any compile-time validation based on the SQL query string.
 * It only supports rusqlite for now.