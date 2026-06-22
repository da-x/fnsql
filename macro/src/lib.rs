//! The `fnsql` crate provides simple type-safe wrappers around SQL queries.
//! Instead of calling type-less `.query()` and `.execute()`, you call auto-generated
//! unique wrappers that are strongly typed: `.query_<name>()` and `.execute_<name>()`.
//! You manually specify the input and output types, but only once, with the query,
//! in separation from the code that uses the query.
//!
//! It's a very simple implementation that doesn't force any schema or ORM down
//! your throat, so if you are already using the `sqlx` or `postgres` crates,
//! you can gradually replace your type-less queries with the type-ful wrappers,
//! or migrate from an opinionated ORM.
//!
//! ## Quick start (sqlx_sqlite)
//!
//! ```rust,no_run
//! fnsql::fnsql! {
//!     #[sqlx_sqlite, test]
//!     create_table_pet() {
//!         "CREATE TABLE pet (
//!               id      INTEGER PRIMARY KEY,
//!               name    TEXT NOT NULL,
//!               data    BLOB
//!         )"
//!     }
//!
//!     #[sqlx_sqlite, test(with=[create_table_pet])]
//!     insert_new_pet(name: String, data: Option<Vec<u8>>) {
//!         "INSERT INTO pet (name, data) VALUES (:name, :data)"
//!     }
//!
//!     #[sqlx_sqlite, test(with=[create_table_pet])]
//!     get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>)] {
//!         "SELECT id, data FROM pet WHERE pet.name = :name"
//!     }
//! }
//! ```
//!
//! The generated methods are extension methods on `sqlx::SqlitePool`:
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), sqlx::Error> {
//! let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
//!
//! // DDL/DML — returns rows affected
//! pool.execute_create_table_pet().await?;
//! pool.execute_insert_new_pet(&"Max".to_string(), &None).await?;
//!
//! // Query returning multiple rows as Vec of typed tuples
//! let rows = pool.query_get_pet_id_data(&Some("Max".to_string())).await?;
//! for (id, data) in rows {
//!     println!("Found pet id={:?}, data={:?}", id, data);
//! }
//!
//! // Query returning exactly one row
//! let (id, data) = pool.query_one_get_pet_id_data(&Some("Max".to_string())).await?;
//!
//! // Query that may return zero or one row
//! let row = pool.query_opt_get_pet_id_data(&Some("Nonexistent".to_string())).await?;
//! # Ok(()) }
//! ```
//!
//! ## Quick start (postgres)
//!
//! For postgres, named parameters are transformed to positional `$1`, `$2`, etc.
//! when the `named` attribute is used:
//!
//! ```rust,no_run
//! fnsql::fnsql! {
//!     #[postgres]
//!     create_table_pet() {
//!         "CREATE TABLE pet (id SERIAL PRIMARY KEY, name TEXT NOT NULL)"
//!     }
//!
//!     #[postgres, named]
//!     insert_new_pet(id: i32, name: String) {
//!         "INSERT INTO pet (id, name) VALUES (:id, :name)"
//!     }
//! }
//! ```
//!
//! Generated methods are extension methods on `postgres::Client` and
//! `postgres::Transaction<'a>`, with both direct and prepared variants:
//!
//! ```rust,no_run
//! # fn example(conn: &mut postgres::Client) -> Result<(), postgres::Error> {
//! conn.execute_create_table_pet()?;
//! conn.execute_insert_new_pet(&1, &"Max".to_string())?;
//!
//! // Prepared statement variant
//! let prep = conn.prepare_insert_new_pet()?;
//! conn.execute_prepared_insert_new_pet(&prep, &2, &"Rex".to_string())?;
//! # Ok(()) }
//! ```
//!
//! ## Generated API
//!
//! ### sqlx_sqlite (`sqlx::SqlitePool`)
//!
//! For each query `<name>(p1: T1, p2: T2) -> [(O1, O2)]`, the following async
//! methods are generated on `sqlx::SqlitePool`:
//!
//! | Method | Return Type | Description |
//! |--------|------------|-------------|
//! | `execute_<name>(&self, &p1, &p2)` | `Result<u64, sqlx::Error>` | Execute, returns rows affected |
//! | `query_<name>(&self, &p1, &p2)` | `Result<Vec<(O1, O2)>, sqlx::Error>` | Fetch all matching rows |
//! | `query_one_<name>(&self, &p1, &p2)` | `Result<(O1, O2), sqlx::Error>` | Fetch exactly one row |
//! | `query_opt_<name>(&self, &p1, &p2)` | `Result<Option<(O1, O2)>, sqlx::Error>` | Fetch zero or one row |
//!
//! Named parameters in the SQL (`:name`) are automatically transformed to
//! positional `$N` placeholders at compile time.
//!
//! A `convert_row_<name>(row: SqliteRow) -> Result<(O1, O2), sqlx::Error>`
//! function is also generated for manual row conversion.
//!
//! ### postgres (`postgres::Client` / `postgres::Transaction<'a>`)
//!
//! For each query `<name>(p1: T1, p2: T2) -> [(O1, O2)]`, the following methods
//! are generated on both `postgres::Client` and `postgres::Transaction<'a>`:
//!
//! | Method | Return Type | Description |
//! |--------|------------|-------------|
//! | `execute_<name>(&mut self, &p1, &p2)` | `Result<u64, postgres::Error>` | Execute directly |
//! | `prepare_<name>(&mut self)` | `Result<<name>Statement_, postgres::Error>` | Prepare statement |
//! | `execute_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<u64, postgres::Error>` | Execute prepared |
//! | `query_<name>(&mut self, &p1, &p2)` | `Result<Vec<(O1, O2)>, postgres::Error>` | Fetch all rows |
//! | `query_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<Vec<(O1, O2)>, postgres::Error>` | Fetch all, prepared |
//! | `query_one_<name>(&mut self, &p1, &p2)` | `Result<(O1, O2), postgres::Error>` | Fetch one row |
//! | `query_one_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<(O1, O2), postgres::Error>` | Fetch one, prepared |
//! | `query_opt_<name>(&mut self, &p1, &p2)` | `Result<Option<(O1, O2)>, postgres::Error>` | Fetch opt row |
//! | `query_opt_prepared_<name>(&mut self, stmt, &p1, &p2)` | `Result<Option<(O1, O2)>, postgres::Error>` | Fetch opt, prepared |
//!
//! With the `prepare-cache` feature enabled, an additional `prepare_cached_<name>()`
//! method is available that uses `fnsql::postgres::Cache`.
//!
//! ## Attributes
//!
//! Each query declaration starts with attributes in square brackets:
//!
//! ```text
//! #[<backend>, <attr2>, <attr3>, ...]
//! ```
//!
//! **Backend (required, exactly one):**
//! - `sqlx_sqlite` — generates async methods on `sqlx::SqlitePool`
//! - `postgres` — generates sync methods on `postgres::Client` / `Transaction`
//!
//! **Optional attributes:**
//! - `test` — generates an auto-test that runs the query with arbitrary values
//! - `test(with=[other_query])` — same as `test`, but runs prerequisite queries first
//! - `named` (postgres only) — transforms `:name` placeholders to `$1`, `$2`, etc.
//! - `conststr=<NAME>` — generates a `pub const NAME: &str` with the query string
//!
//! ## Parameters and return types
//!
//! **Parameters** use Rust-like syntax: `param_name: Type`. The generated methods
//! accept references: `&param_name`. Supported types are anything that implements
//! the backend's respective trait (`sqlx::Encode` for sqlx, `postgres::types::ToSql`
//! for postgres).
//!
//! Common type shortcuts:
//! - `str` is accepted as a parameter type (maps to `&str`)
//! - `[u8]` is accepted as a parameter type (maps to `&[u8]`)
//!
//! **Return types** are optional and specified as `-> [(T1, T2, ...)]`. Each type
//! corresponds to a column in the result set. If omitted, the query is treated as
//! returning no data (DDL/DML).
//!
//! ## Auto-generated tests
//!
//! With the `test` attribute, fnsql generates a `#[test]` (or `#[tokio::test]` for
//! sqlx_sqlite) that opens an in-memory database, runs prerequisite queries via
//! `test(with=[...])`, and executes the query with arbitrary values. This validates
//! that your query syntax is correct without writing any test code.
//!
//! To enable test compilation, add to your `[dev-dependencies]`:
//!
//! ```toml
//! arbitrary = { version = "1", features = ["derive"] }
//! ```


extern crate proc_macro;

use std::collections::HashMap;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{quote, ToTokens};
use regex::{Captures, Regex};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Ident, LitStr, Token,
};

struct Queries {
    list: Vec<Query>,
}

impl Parse for Queries {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut list = vec![];
        while !input.is_empty() {
            list.push(input.parse()?)
        }

        Ok(Queries { list })
    }
}

enum Kind {
    SqlxSqlite,
    PostgreSQL,
}

struct Query {
    name: Ident,
    params: Vec<Param>,
    outputs: Vec<Output>,
    query: syn::LitStr,
    kind: Kind,
    test: Option<Vec<String>>,
    named: bool,
    conststr: Option<String>,
}

impl Parse for Query {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut kind = None;
        let mut test = None;
        let mut named = false;
        let mut conststr = None;

        if input.peek(Token![#]) {
            let _: Token![#] = input.parse()?;
            let content;
            let _ = bracketed!(content in input);
            let list: Punctuated<Attr, Token![,]> = content.parse_terminated(Parse::parse)?;

            for attr in list {
                match attr {
                    Attr::Kind(attr_kind) => {
                        kind = Some(attr_kind);
                    }
                    Attr::Test(test_attrs) => {
                        if test.is_none() {
                            test = Some(vec![]);
                        }
                        for test_attr in test_attrs {
                            match test_attr {
                                TestAttr::With(v) => {
                                    test.as_mut().unwrap().extend(v);
                                }
                            }
                        }
                    }
                    Attr::Named => {
                        named = true;
                    }
                    Attr::ConstStr(v) => {
                        conststr = Some(v);
                    }
                }
            }
        };

        let name = input.parse()?;
        let kind = match kind {
            None => panic!("unknown SQL type. Supported: sqlx_sqlite, postgres"),
            Some(kind) => kind,
        };
        let content;
        let _ = parenthesized!(content in input);
        let list: Punctuated<_, Token![,]> = content.parse_terminated(Parse::parse)?;
        let params = list.into_iter().collect();

        let outputs = if input.peek(Token![->]) {
            let _: Token![->] = input.parse()?;

            let content;
            let _ = bracketed!(content in input);
            {
                let sub_content;
                let _ = parenthesized!(sub_content in content);
                let list: Punctuated<_, Token![,]> = sub_content.parse_terminated(Parse::parse)?;
                list.into_iter().collect()
            }
        } else {
            vec![]
        };

        let content;
        let _ = braced!(content in input);
        let query = content.parse::<syn::LitStr>()?;

        Ok(Query {
            name,
            params,
            outputs,
            query,
            kind,
            test,
            named,
            conststr,
        })
    }
}

impl Query {
    fn prepend_name(&self, prefix: &'static str) -> Ident {
        Ident::new(&format!("{}{}", prefix, &self.name), self.name.span())
    }

    fn params_declr(&self) -> Tokens {
        let list: Vec<_> = self.params.iter().map(|x| x.expand_declr()).collect();
        quote! { #(, #list)* }
    }

    fn outputs_declr(&self) -> Tokens {
        let list: Vec<_> = self.outputs.iter().map(|x| x.expand_declr()).collect();
        quote! { #(#list),* }
    }

    fn outputs_row_try_get_numbered(&self) -> Tokens {
        let list: Vec<_> = self
            .outputs
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let i = syn::LitInt::new(&format!("{}", i), self.name.span());
                quote! {row.try_get(#i)?}
            })
            .collect();

        quote! { #(#list),* }
    }

    fn outputs_row_try_get_numbered_typed(&self) -> Tokens {
        let list: Vec<_> = self
            .outputs
            .iter()
            .enumerate()
            .map(|(i, out)| {
                let i = syn::LitInt::new(&format!("{}", i), self.name.span());
                let ttype = &out.ttype;
                quote! {{ use sqlx::Row; row.try_get::<#ttype, _>(#i)? }}
            })
            .collect();

        quote! { #(#list),* }
    }

    fn params_arbitrary(&self) -> (Tokens, Tokens) {
        let mut gen_lets = vec![];
        let mut params = vec![];

        let _ = self
            .params
            .iter()
            .enumerate()
            .map(|(idx, param)| {
                let ttype = &param.ttype;
                let owned_ttype = if ttype.to_token_stream().to_string() == "str" {
                    quote! {String}
                } else if ttype.to_token_stream().to_string() == "[u8]" {
                    quote! {Vec<u8>}
                } else {
                    quote! {#ttype}
                };
                let ident = Ident::new(&format!("i_{}", idx), self.name.span());

                gen_lets.push(quote! {
                    let #ident: #owned_ttype = arbitrary::Arbitrary::arbitrary(uns).unwrap();
                });
                params.push(quote! {&#ident});
            })
            .collect::<Vec<()>>();

        (quote! { #(#gen_lets);* }, quote! { #(#params),* })
    }

    fn params_query_ref(&self) -> Tokens {
        let list: Vec<_> = self.params.iter().map(|x| x.expand_query(self)).collect();
        if list.len() == 0 {
            quote! { &[] }
        } else {
            quote! { &[#(#list),*] }
        }
    }

    fn expand(&self) -> Tokens {
        match self.kind {
            Kind::SqlxSqlite => self.sqlx_sqlite_expand(),
            Kind::PostgreSQL => self.postgres_expand(),
        }
    }

    fn postgres_expand(&self) -> Tokens {
        #[allow(non_snake_case)]
        let Client = self.prepend_name("Client_");
        #[allow(non_snake_case)]
        let Statement = self.prepend_name("Statement_");
        let execute_name = self.prepend_name("execute_");
        let execute_prepared_name = self.prepend_name("execute_prepared_");
        let prepare_name = self.prepend_name("prepare_");
        #[allow(unused_variables)]
        let prepare_cached_name = self.prepend_name("prepare_cached_");
        let convert_row = self.prepend_name("convert_row_");
        let query_name = self.prepend_name("query_");
        let query_prepared_name = self.prepend_name("query_prepared_");
        let query_one_name = self.prepend_name("query_one_");
        let query_one_prepared_name = self.prepend_name("query_one_prepared_");
        let query_opt_name = self.prepend_name("query_opt_");
        let query_opt_prepared_name = self.prepend_name("query_opt_prepared_");
        let params_declr = self.params_declr();
        let params_query_ref = self.params_query_ref();
        let outputs_declr = self.outputs_declr();
        let row_try_get_numbered = self.outputs_row_try_get_numbered();

        let query;
        if self.named {
            lazy_static::lazy_static! {
                static ref RE: Regex = Regex::new(":([A-Za-z_][_A-Za-z0-9]*)($|[^_A-Za-z0-9])").unwrap();
            }

            let params: HashMap<_, _> = self
                .params
                .iter()
                .enumerate()
                .map(|(idx, param)| (format!("{}", param.name), idx))
                .collect();

            query = String::from(RE.replace_all(&self.query.value(), |captures: &Captures| {
                let c1 = captures.get(1).unwrap().as_str();
                let c2 = captures.get(2).unwrap().as_str();
                match params.get(c1) {
                    Some(idx) => format!("${}{}", idx + 1, c2),
                    None => format!("{}{}", c1, c2),
                }
            }));
        } else {
            query = self.query.value();
        };
        let query = LitStr::new(query.as_str(), self.query.span());

        let const_str = self.conststr.as_ref().map(|name| {
            let ident = Ident::new(name, Span::call_site());
            quote! { pub const #ident: &str = #query; }
        });

        #[cfg(feature = "prepare-cache")]
        let (prepare_cached_decl, prepare_cached_impl) = {
            let prepare_cached_decl = quote! {
                fn #prepare_cached_name(&mut self, cache: &mut fnsql::postgres::Cache) -> Result<#Statement, postgres::Error>;
            };

            let prepare_cached_impl = quote! {
                fn #prepare_cached_name(&mut self, cache: &mut fnsql::postgres::Cache) -> Result<#Statement, postgres::Error> {
                    Ok(#Statement(cache.prepare(#query, self)?))
                }
            };

            (prepare_cached_decl, prepare_cached_impl)
        };

        #[cfg(not(feature = "prepare-cache"))]
        let (prepare_cached_decl, prepare_cached_impl) = { (quote! {}, quote! {}) };

        let defs = quote! {
            #[allow(non_camel_case_types)]
            pub struct #Statement(pub postgres::Statement);

            #[allow(non_camel_case_types)]
            pub trait #Client {
                fn #prepare_name(&mut self) -> Result<#Statement, postgres::Error>;
                #prepare_cached_decl
                fn #execute_name(&mut self #params_declr) -> Result<u64, postgres::Error>;
                fn #execute_prepared_name(&mut self, stmt: &#Statement #params_declr)
                    -> Result<u64, postgres::Error>;
                fn #query_name(&mut self #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error>;
                fn #query_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error>;
                fn #query_one_name(&mut self #params_declr) -> Result<(#outputs_declr), postgres::Error>;
                fn #query_one_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<(#outputs_declr), postgres::Error>;
                fn #query_opt_name(&mut self #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error>;
                fn #query_opt_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error>;
            }

            pub fn #convert_row(row: postgres::Row) -> Result<(#outputs_declr), postgres::Error> {
                Ok((#row_try_get_numbered))
            }
        };

        let timpl = quote! {
            fn #prepare_name(&mut self)  -> Result<#Statement, postgres::Error> {
                self.prepare(#query).map(#Statement)
            }

            #prepare_cached_impl

            fn #execute_name(&mut self #params_declr) -> Result<u64, postgres::Error> {
                self.execute(#query, #params_query_ref)
            }

            fn #execute_prepared_name(&mut self, stmt: &#Statement #params_declr)
                -> Result<u64, postgres::Error>
            {
                self.execute(&stmt.0, #params_query_ref)
            }

            fn #query_name(&mut self #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error> {
                let result: Result<Vec<_>, postgres::Error> =
                    self.query(#query, #params_query_ref)?.into_iter().map(#convert_row).collect();
                result
            }

            fn #query_one_name(&mut self #params_declr) -> Result<(#outputs_declr), postgres::Error> {
                Ok(#convert_row(self.query_one(#query, #params_query_ref)?)?)
            }

            fn #query_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error> {
                let result: Result<Vec<_>, postgres::Error> =
                    self.query(&stmt.0, #params_query_ref)?.into_iter().map(#convert_row).collect();
                result
            }

            fn #query_one_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<(#outputs_declr), postgres::Error> {
                Ok(#convert_row(self.query_one(&stmt.0, #params_query_ref)?)?)
            }

            fn #query_opt_name(&mut self #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error> {
                match self.query_opt(#query, #params_query_ref)? {
                    None => Ok(None),
                    Some(x) => Ok(Some(#convert_row(x)?)),
                }
            }

            fn #query_opt_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error> {
                match self.query_opt(&stmt.0, #params_query_ref)? {
                    None => Ok(None),
                    Some(x) => Ok(Some(#convert_row(x)?)),
                }
            }
        };

        let test_code = self.test_code();

        quote! {
            #const_str
            #defs

            impl #Client for postgres::Client {
                #timpl
            }

            impl<'a> #Client for postgres::Transaction<'a> {
                #timpl
            }

            #test_code
        }
    }

    fn sqlx_bind_chain(&self) -> Tokens {
        let binds: Vec<_> = self.params.iter().map(|p| {
            let name = &p.name;
            quote! { .bind(#name) }
        }).collect();
        quote! { #(#binds)* }
    }

    fn sqlx_sqlite_expand(&self) -> Tokens {
        #[allow(non_snake_case)]
        let PoolTrait = self.prepend_name("Pool_");
        let execute_name = self.prepend_name("execute_");
        let convert_row = self.prepend_name("convert_row_");
        let query_name = self.prepend_name("query_");
        let query_one_name = self.prepend_name("query_one_");
        let query_opt_name = self.prepend_name("query_opt_");
        let params_declr = self.params_declr();
        let outputs_declr = self.outputs_declr();
        let row_try_get_numbered = self.outputs_row_try_get_numbered_typed();
        let bind_chain = self.sqlx_bind_chain();

        // Transform :name placeholders to $1, $2, ... for sqlx sqlite
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(":([A-Za-z_][_A-Za-z0-9]*)($|[^_A-Za-z0-9])").unwrap();
        }
        let params: HashMap<_, _> = self
            .params
            .iter()
            .enumerate()
            .map(|(idx, param)| (format!("{}", param.name), idx))
            .collect();
        let query_str = String::from(RE.replace_all(&self.query.value(), |captures: &Captures| {
            let c1 = captures.get(1).unwrap().as_str();
            let c2 = captures.get(2).unwrap().as_str();
            match params.get(c1) {
                Some(idx) => format!("${}{}", idx + 1, c2),
                None => format!("{}{}", c1, c2),
            }
        }));
        let query = LitStr::new(&query_str, self.query.span());

        let const_str = self.conststr.as_ref().map(|name| {
            let ident = Ident::new(name, Span::call_site());
            quote! { pub const #ident: &str = #query; }
        });

        #[allow(non_snake_case)]
        let TxTrait = self.prepend_name("Tx_");

        let test_code = self.test_code();

        quote! {
            #const_str
            #[allow(non_camel_case_types)]
            pub trait #PoolTrait {
                async fn #execute_name(&self #params_declr) -> Result<u64, sqlx::Error>;
                async fn #query_name(&self #params_declr) -> Result<Vec<(#outputs_declr)>, sqlx::Error>;
                async fn #query_one_name(&self #params_declr) -> Result<(#outputs_declr), sqlx::Error>;
                async fn #query_opt_name(&self #params_declr) -> Result<Option<(#outputs_declr)>, sqlx::Error>;
            }

            #[allow(non_camel_case_types)]
            pub trait #TxTrait {
                async fn #execute_name(&mut self #params_declr) -> Result<u64, sqlx::Error>;
                async fn #query_name(&mut self #params_declr) -> Result<Vec<(#outputs_declr)>, sqlx::Error>;
                async fn #query_one_name(&mut self #params_declr) -> Result<(#outputs_declr), sqlx::Error>;
                async fn #query_opt_name(&mut self #params_declr) -> Result<Option<(#outputs_declr)>, sqlx::Error>;
            }

            pub fn #convert_row(row: sqlx::sqlite::SqliteRow) -> Result<(#outputs_declr), sqlx::Error> {
                Ok((#row_try_get_numbered))
            }

            impl #PoolTrait for sqlx::SqlitePool {
                async fn #execute_name(&self #params_declr) -> Result<u64, sqlx::Error> {
                    sqlx::query(#query)#bind_chain.execute(self).await.map(|r| r.rows_affected())
                }

                async fn #query_name(&self #params_declr) -> Result<Vec<(#outputs_declr)>, sqlx::Error> {
                    let rows = sqlx::query(#query)#bind_chain.fetch_all(self).await?;
                    rows.into_iter().map(#convert_row).collect()
                }

                async fn #query_one_name(&self #params_declr) -> Result<(#outputs_declr), sqlx::Error> {
                    let row = sqlx::query(#query)#bind_chain.fetch_one(self).await?;
                    #convert_row(row)
                }

                async fn #query_opt_name(&self #params_declr) -> Result<Option<(#outputs_declr)>, sqlx::Error> {
                    match sqlx::query(#query)#bind_chain.fetch_optional(self).await? {
                        None => Ok(None),
                        Some(row) => Ok(Some(#convert_row(row)?)),
                    }
                }
            }

            impl<'c> #TxTrait for sqlx::Transaction<'c, sqlx::Sqlite> {
                async fn #execute_name(&mut self #params_declr) -> Result<u64, sqlx::Error> {
                    sqlx::query(#query)#bind_chain.execute(&mut **self).await.map(|r| r.rows_affected())
                }

                async fn #query_name(&mut self #params_declr) -> Result<Vec<(#outputs_declr)>, sqlx::Error> {
                    let rows = sqlx::query(#query)#bind_chain.fetch_all(&mut **self).await?;
                    rows.into_iter().map(#convert_row).collect()
                }

                async fn #query_one_name(&mut self #params_declr) -> Result<(#outputs_declr), sqlx::Error> {
                    let row = sqlx::query(#query)#bind_chain.fetch_one(&mut **self).await?;
                    #convert_row(row)
                }

                async fn #query_opt_name(&mut self #params_declr) -> Result<Option<(#outputs_declr)>, sqlx::Error> {
                    match sqlx::query(#query)#bind_chain.fetch_optional(&mut **self).await? {
                        None => Ok(None),
                        Some(row) => Ok(Some(#convert_row(row)?)),
                    }
                }
            }

            #test_code
        }
    }

    fn test_code(&self) -> Tokens {
        let test_name = self.prepend_name("auto_");
        let testsetup_name = self.prepend_name("testsetup_");
        let (params_arbit_prep, params_arbit) = self.params_arbitrary();
        let execute_name = self.prepend_name("execute_");
        let name = syn::LitStr::new(&self.name.to_string(), self.name.span());

        let client_type = match self.kind {
            Kind::SqlxSqlite => quote! {sqlx::SqlitePool},
            Kind::PostgreSQL => quote! {postgres::Client},
        };
        let client_ref_type = match self.kind {
            Kind::SqlxSqlite => quote! {&},
            Kind::PostgreSQL => quote! {&mut},
        };
        let ignore_error = match self.kind {
            Kind::SqlxSqlite => quote! {},
            Kind::PostgreSQL => quote! {},
        };
        let error_type = match self.kind {
            Kind::SqlxSqlite => quote! {sqlx::Error},
            Kind::PostgreSQL => quote! {postgres::Error},
        };
        let open_client = match self.kind {
            Kind::SqlxSqlite => quote! {
                let conn = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
            },
            Kind::PostgreSQL => quote! {let mut conn = {
                let mut conn = fnsql::postgres::testing_client().expect("unable to connect testing client");
                conn.execute("SET search_path TO pg_temp", &[]).unwrap();
                conn
            }; },
        };

        let test = if let Some(depends) = &self.test {
            let depends = depends.iter().map(|name| {
                let parent_testsetup_name =
                    Ident::new(&format!("testsetup_{}", name), self.name.span());
                match self.kind {
                    Kind::SqlxSqlite => quote! {
                        #parent_testsetup_name(uns, deps, conn).await?;
                    },
                    _ => quote! {
                        #parent_testsetup_name(uns, deps, conn)?;
                    },
                }
            });

            let exec_call = match self.kind {
                Kind::SqlxSqlite => quote! {
                    let r = conn.#execute_name(#params_arbit).await;
                },
                _ => quote! {
                    let r = conn.#execute_name(#params_arbit);
                },
            };

            let testsetup_body_inner = quote! {
                if !deps.insert(#name) {
                    return Ok(());
                }

                #(#depends);*

                #params_arbit_prep;
                #exec_call
                match r {
                    Ok(_) => {}
                    #ignore_error
                    Err(err) => {
                        eprintln!("{:?}", err);
                        Err(err)?;
                    },
                }
                Ok(())
            };

            match self.kind {
                Kind::SqlxSqlite => {
                    quote! {
                        #[cfg(test)]
                        async fn #testsetup_name(
                            uns: &mut arbitrary::Unstructured<'_>,
                            deps: &mut std::collections::HashSet<&'static str>,
                            conn: #client_ref_type #client_type) -> Result<(), #error_type>
                        {
                            #testsetup_body_inner
                        }

                        #[tokio::test]
                        async fn #test_name() -> Result<(), #error_type> {
                            #open_client
                            let mut deps = std::collections::HashSet::new();
                            let raw_data: &[u8] = &[1, 2, 3];
                            let mut unstructured = arbitrary::Unstructured::new(raw_data);

                            #testsetup_name(&mut unstructured, &mut deps, #client_ref_type conn).await?;
                            Ok(())
                        }
                    }
                }
                _ => {
                    quote! {
                        #[cfg(test)]
                        fn #testsetup_name(
                            uns: &mut arbitrary::Unstructured<'_>,
                            deps: &mut std::collections::HashSet<&'static str>,
                            conn: #client_ref_type #client_type) -> Result<(), #error_type>
                        {
                            #testsetup_body_inner
                        }

                        #[test]
                        fn #test_name() -> Result<(), #error_type> {
                            #open_client;
                            let mut deps = std::collections::HashSet::new();
                            let raw_data: &[u8] = &[1, 2, 3];
                            let mut unstructured = arbitrary::Unstructured::new(raw_data);

                            #testsetup_name(&mut unstructured, &mut deps, #client_ref_type conn)?;
                            Ok(())
                        }
                    }
                }
            }
        } else {
            quote! {}
        };
        test
    }
}

struct Output {
    ttype: syn::Type,
}

impl Parse for Output {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ttype = input.parse()?;

        Ok(Self { ttype })
    }
}

impl Output {
    fn expand_declr(&self) -> Tokens {
        let ttype = &self.ttype;

        quote! { #ttype }
    }
}

struct Param {
    name: Ident,
    ttype: syn::Type,
}

impl Parse for Param {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let _: Token![:] = input.parse()?;
        let ttype = input.parse()?;

        Ok(Self { name, ttype })
    }
}

impl Param {
    fn expand_declr(&self) -> Tokens {
        let name = &self.name;
        let ttype = &self.ttype;

        quote! { #name: &#ttype }
    }

    fn expand_query(&self, query: &Query) -> Tokens {
        let name = &self.name;

        match query.kind {
            Kind::SqlxSqlite => unreachable!("sqlx uses sqlx_bind_chain"),
            Kind::PostgreSQL => quote! { &#name as &(dyn postgres::types::ToSql + Sync) },
        }
    }
}

enum Attr {
    Kind(Kind),
    Test(Vec<TestAttr>),
    Named,
    ConstStr(String),
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident == "sqlx_sqlite" {
            return Ok(Attr::Kind(Kind::SqlxSqlite));
        }
        if ident == "postgres" {
            return Ok(Attr::Kind(Kind::PostgreSQL));
        }
        if ident == "named" {
            return Ok(Attr::Named);
        }
        if ident == "test" {
            let mut v = vec![];

            if input.peek(token::Paren) {
                let content;
                let _ = parenthesized!(content in input);
                let list: Punctuated<TestAttr, Token![,]> =
                    content.parse_terminated(Parse::parse)?;
                v = list.into_iter().collect();
            };

            return Ok(Attr::Test(v));
        }
        if ident == "conststr" {
            let _: Token![=] = input.parse()?;
            let name: Ident = input.parse()?;
            return Ok(Attr::ConstStr(name.to_string()));
        }
        panic!("unknown attribute {}", ident);
    }
}

enum TestAttr {
    With(Vec<String>),
}

impl Parse for TestAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident == "with" {
            let mut v = vec![];

            let _: Token![=] = input.parse()?;
            let content;
            let _ = bracketed!(content in input);
            let list: Punctuated<Ident, Token![,]> = content.parse_terminated(Parse::parse)?;
            for item in list {
                v.push(item.to_string());
            }

            return Ok(TestAttr::With(v));
        }

        panic!("unknown test attribute {}", ident);
    }
}

/// Declares type-safe SQL query wrappers.
///
/// Each declaration consists of:
/// 1. Attributes in `#[...]`: backend + optional flags
/// 2. A name and parameters: `<name>(param: Type, ...)`
/// 3. Optional return type: `-> [(ColType1, ColType2, ...)]`
/// 4. The SQL string in braces: `{ "..." }`
///
/// ```ignore
/// fnsql! {
///     #[sqlx_sqlite, test]
///     get_user(id: i32) -> [(String, i32)] {
///         "SELECT name, age FROM users WHERE id = :id"
///     }
///
///     #[postgres, named, test(with=[get_user])]
///     update_user_name(id: i32, name: String) {
///         "UPDATE users SET name = :name WHERE id = :id"
///     }
/// }
/// ```
///
/// See the [crate-level documentation](index.html) for full details on generated
/// methods, attributes, and type conventions.

#[proc_macro]
pub fn fnsql(input: TokenStream) -> TokenStream {
    let queries: Queries = parse_macro_input!(input);
    let queries: Vec<_> = queries.list.iter().map(|x| x.expand()).collect();

    quote! { #(#queries)* }.into()
}
