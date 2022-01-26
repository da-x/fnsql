# fnsql &emsp; [![Build Status]][travis] [![Latest Version]][crates.io] [![Docs badge]][Docs link] [![License badge]][License link]

[Build Status]: https://api.travis-ci.org/da-x/fnsql.svg?branch=master
[travis]: https://travis-ci.org/da-x/fnsql
[Latest Version]: https://img.shields.io/crates/v/fnsql.svg
[crates.io]: https://crates.io/crates/fnsql
[License badge]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg
[License link]: https://travis-ci.org/da-x/fnsql
[Docs badge]: https://docs.rs/fnsql/badge.svg
[Docs link]: https://docs.rs/fnsql

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

```rust
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

The idea with this crate is to encourage SQL usage but never use inline queries
or have type inference at the call-site. Instead, we declare each query on
top-level, giving a name.

- The types of named variables are give in a Rust-like syntax.
- The type of the returned row is also provided.
- `fnsql` does not make an assurances to make sure the types match the query,
  you will discover it with `cargo test` and no additional code.
- `fnsql` writes the tests for each of the queries.  - `Arbitrary` is used to
  generate parameter values.
- If testing one query depend on another, you can specify that with `test(with=[..])`.

```
running 3 tests
test auto_create_table_pet ... ok
test auto_insert_new_pet ... ok
test auto_get_pet_id_data ... ok
```

## Limitations

 * Does not do any compile-time validation based on SQL query string.
 * Only supports rusqlite


## License

`fnsql` is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `fnsql` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.