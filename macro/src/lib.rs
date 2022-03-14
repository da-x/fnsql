//! The `fnsql` crate provides simple type-safe optional wrappers around SQL
//! queries. Instead of calling type-less `.query()` and `.execute()`, you call to
//! auto-generated unique wrappers that are strongly typed, `.query_<name>()` and
//! `.execute_<name>()`. However, you manually specify the input and output types,
//! but only once, with the query, and in separation with the code that uses the
//! query.
//!
//! It's a very simple implementation that doesn't force any schema or ORM down
//! your throat, so if you are already using the `rusqlite` or `postgres` crates,
//! you can gradually replace your type-less queries with the type-ful wrappers,
//! or migrate from an opinionated ORM.
//!
//! The way to generate these wrappers is to specify input and output types for
//! each one of the queries. For example, consider the following definitions
//! specified with `fnsql`, based on the `rusqlite` example:
//!
//! ```rust
//! fnsql::fnsql! {
//!     #[rusqlite, test]
//!     create_table_pet() {
//!         "CREATE TABLE pet (
//!               id      INTEGER PRIMARY KEY,
//!               name    TEXT NOT NULL,
//!               data    BLOB
//!         )"
//!     }
//!
//!     #[rusqlite, test(with=[create_table_pet])]
//!     insert_new_pet(name: String, data: Option<Vec<u8>>) {
//!         "INSERT INTO pet (name, data) VALUES (:name, :data)"
//!     }
//!
//!     #[rusqlite, test(with=[create_table_pet])]
//!     get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>, String)] {
//!         "SELECT id, data, name FROM pet WHERE pet.name = :name"
//!     }
//! }
//! ```
//!
//! The definitions can be used as such (commented out is how the previous
//! type-less interfaces were used):
//!
//! ```rust ignore
//! let mut conn = rusqlite::Connection::open_in_memory()?;
//!
//! conn.execute_create_table_pet()?;
//! // conn.execute(
//! //    "CREATE TABLE pet (
//! //               id              INTEGER PRIMARY KEY,
//! //               name            TEXT NOT NULL,
//! //               data            BLOB
//! //               )",
//! //     [],
//! // )?;
//!
//! conn.execute_insert_new_pet(&me.name, &me.data)?;
//! // conn.execute(
//! //     "INSERT INTO pet (name, data) VALUES (?1, ?2)",
//! //     params![me.name, me.data],
//! // )?;
//!
//! let mut stmt = conn.prepare_get_pet_id_data()?;
//! // let mut stmt = conn.prepare("SELECT id, data, name FROM pet WHERE pet.name = :name")?;
//!
//! let pet_iter = stmt.query_map(&Some("Max".to_string()), |id, data, name| {
//!     Ok::<_, rusqlite::Error>(Pet {
//!         id,
//!         data,
//!         name,
//!     })
//! })?;
//! // let pet_iter = stmt.query_map([(":name", "Max".to_string())], |row| {
//! //     Ok(Pet {
//! //         id: row.get(0)?,
//! //         name: row.get(1)?,
//! //         data: row.get(2)?,
//! //     })
//! // })?;
//! ```
//!
//! ## Technical discussion
//!
//! The idea with this crate is to allow direct SQL usage but never use inline
//! queries or have type inference at the call-site. Instead, we declare each query
//! on top-level, giving each a name and designated accessor methods that derive
//! from the name.
//!
//! - The types of named variables are give in a Rust-like syntax.
//! - The type of the returned row is also provided.
//! - `fnsql` does not make an assurances to make sure the types match the query,
//!   you will discover it with `cargo test` and no additional code.
//! - `fnsql` writes the tests for each of the queries.  - `Arbitrary` is used to
//!   generate parameter values.
//! - If testing one query depend on another, you can specify that with `test(with=[..])`.
//!
//! ```text
//! running 3 tests
//! test auto_create_table_pet ... ok
//! test auto_insert_new_pet ... ok
//! test auto_get_pet_id_data ... ok
//! ```
//!
//! The following is for allowing generated query tests to compile:
//!
//! ```toml
//! [dev-dependencies]
//! arbitrary = { version = "1", features = ["derive"] }
//! ```
//!
//! ## Limitations
//!
//!  * Though it <i>does</i> provide auto-generated tests for validating queries in `cargo test`,
//!    it does not do any compile-time validation based on the SQL query string.
//!  * It only supports rusqlite for now.

extern crate proc_macro;

use std::collections::HashMap;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{quote, ToTokens};
use regex::{Regex, Captures};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Ident, Token, LitStr,
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
    Rusqlite,
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
}

impl Parse for Query {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut kind = None;
        let mut test = None;
        let mut named = false;

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
                    },
                }
            }
        };

        let name = input.parse()?;
        let kind = match kind {
            None => panic!("unknown SQL type. Supported: rusqlite"),
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

    fn outputs_row_get_numbered(&self) -> Tokens {
        let list: Vec<_> = self
            .outputs
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let i = syn::LitInt::new(&format!("{}", i), self.name.span());
                quote! {row.get(#i)?}
            })
            .collect();

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

    fn outputs_mapped_row_closure(&self) -> Tokens {
        let list = self.outputs_row_get_numbered();
        quote! { Ok(map(#list)) }
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

    fn params_query(&self) -> Tokens {
        let list: Vec<_> = self.params.iter().map(|x| x.expand_query(self)).collect();
        if list.len() == 0 {
            quote! { [] }
        } else {
            quote! { &[#(#list),*] }
        }
    }

    fn params_query_ref(&self) -> Tokens {
        let list: Vec<_> = self.params.iter().map(|x| x.expand_query(self)).collect();
        if list.len() == 0 {
            quote! { &[] }
        } else {
            quote! { &[#(#list),*] }
        }
    }

    fn params_relay(&self) -> Tokens {
        let list: Vec<_> = self
            .params
            .iter()
            .map(|x| {
                let name = &x.name;
                quote! { #name }
            })
            .collect();
        if list.len() == 0 {
            quote! {}
        } else {
            quote! { #(#list),*, }
        }
    }

    fn expand(&self) -> Tokens {
        match self.kind {
            Kind::Rusqlite => self.sqlite_expand(),
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
        let convert_row = self.prepend_name("convert_row_");
        let queue_name = self.prepend_name("queue_");
        let queue_prepared_name = self.prepend_name("queue_prepared_");
        let queue_one_name = self.prepend_name("queue_one_");
        let queue_one_prepared_name = self.prepend_name("queue_one_prepared_");
        let queue_opt_name = self.prepend_name("queue_opt_");
        let queue_opt_prepared_name = self.prepend_name("queue_opt_prepared_");
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
                .map(|(idx, param)| {
                    (format!("{}", param.name), idx)
                }).collect();

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

        let defs = quote! {
            #[allow(non_camel_case_types)]
            pub struct #Statement(pub postgres::Statement);

            #[allow(non_camel_case_types)]
            pub trait #Client {
                fn #prepare_name(&mut self) -> Result<#Statement, postgres::Error>;
                fn #execute_name(&mut self #params_declr) -> Result<u64, postgres::Error>;
                fn #execute_prepared_name(&mut self, stmt: &#Statement #params_declr)
                    -> Result<u64, postgres::Error>;
                fn #queue_name(&mut self #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error>;
                fn #queue_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error>;
                fn #queue_one_name(&mut self #params_declr) -> Result<(#outputs_declr), postgres::Error>;
                fn #queue_one_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<(#outputs_declr), postgres::Error>;
                fn #queue_opt_name(&mut self #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error>;
                fn #queue_opt_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error>;
            }

            pub fn #convert_row(row: postgres::Row) -> Result<(#outputs_declr), postgres::Error> {
                Ok((#row_try_get_numbered))
            }
        };

        let timpl = quote! {
            fn #prepare_name(&mut self)  -> Result<#Statement, postgres::Error> {
                self.prepare(#query).map(#Statement)
            }

            fn #execute_name(&mut self #params_declr) -> Result<u64, postgres::Error> {
                self.execute(#query, #params_query_ref)
            }

            fn #execute_prepared_name(&mut self, stmt: &#Statement #params_declr)
                -> Result<u64, postgres::Error>
            {
                self.execute(&stmt.0, #params_query_ref)
            }

            fn #queue_name(&mut self #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error> {
                let result: Result<Vec<_>, postgres::Error> =
                    self.query(#query, #params_query_ref)?.into_iter().map(#convert_row).collect();
                result
            }

            fn #queue_one_name(&mut self #params_declr) -> Result<(#outputs_declr), postgres::Error> {
                Ok(#convert_row(self.query_one(#query, #params_query_ref)?)?)
            }

            fn #queue_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Vec<(#outputs_declr)>, postgres::Error> {
                let result: Result<Vec<_>, postgres::Error> =
                    self.query(&stmt.0, #params_query_ref)?.into_iter().map(#convert_row).collect();
                result
            }

            fn #queue_one_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<(#outputs_declr), postgres::Error> {
                Ok(#convert_row(self.query_one(&stmt.0, #params_query_ref)?)?)
            }

            fn #queue_opt_name(&mut self #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error> {
                match self.query_opt(#query, #params_query_ref)? {
                    None => Ok(None),
                    Some(x) => Ok(Some(#convert_row(x)?)),
                }
            }

            fn #queue_opt_prepared_name(&mut self, stmt: &#Statement #params_declr) -> Result<Option<(#outputs_declr)>, postgres::Error> {
                match self.query_opt(&stmt.0, #params_query_ref)? {
                    None => Ok(None),
                    Some(x) => Ok(Some(#convert_row(x)?)),
                }
            }
        };

        let test_code = self.test_code();

        quote! {
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

    fn sqlite_expand(&self) -> Tokens {
        let conn_trait_name = self.prepend_name("Connection_");
        #[allow(non_snake_case)]
        let StatementType = self.prepend_name("Statement_");
        #[allow(non_snake_case)]
        let CachedStatementType = self.prepend_name("CachedStatement_");
        #[allow(non_snake_case)]
        let MappedRows = self.prepend_name("MappedRows_");
        #[allow(non_snake_case)]
        let Rows = self.prepend_name("Rows_");
        let prepare_name = self.prepend_name("prepare_");
        let prepare_cached_name = self.prepend_name("prepare_cached_");
        let execute_name = self.prepend_name("execute_");
        let query_row_name = self.prepend_name("query_row_");
        let params_declr = self.params_declr();
        let outputs_declr = self.outputs_declr();
        let row_closure = self.outputs_row_get_numbered();
        let mapped_row_closure = self.outputs_mapped_row_closure();
        let params_query = self.params_query();
        let params_relay = self.params_relay();
        let query = &self.query;

        let test_code = self.test_code();

        quote! {
            #[allow(non_camel_case_types)]
            pub trait #conn_trait_name {
                fn #prepare_name(&self) -> rusqlite::Result<#StatementType<'_>>;
                fn #prepare_cached_name(&self) -> rusqlite::Result<#CachedStatementType<'_>>;
                fn #execute_name(&self #params_declr) -> rusqlite::Result<usize>;
                fn #query_row_name<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<T>
                where
                    F: FnMut(#outputs_declr) -> T;
            }

            impl #conn_trait_name for rusqlite::Connection {
                fn #prepare_name(&self) -> rusqlite::Result<#StatementType<'_>> {
                    self.prepare(#query).map(#StatementType)
                }

                fn #prepare_cached_name(&self) -> rusqlite::Result<#CachedStatementType<'_>> {
                    self.prepare_cached(#query).map(#CachedStatementType)
                }

                fn #execute_name(&self #params_declr) -> rusqlite::Result<usize> {
                    self.execute(#query, #params_query)
                }

                fn #query_row_name<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<T>
                where
                    F: FnMut(#outputs_declr) -> T,
                {
                    let mut stmt = self.#prepare_name()?;
                    stmt.query_row(#params_relay f)
                }
            }

            #[allow(non_camel_case_types)]
            pub struct #MappedRows<'stmt, F> {
                rows: rusqlite::Rows<'stmt>,
                map: F,
            }

            impl<'stmt, T, F> #MappedRows<'stmt, F>
            where
                F: FnMut(#outputs_declr) -> T
            {
                pub(crate) fn new(rows: rusqlite::Rows<'stmt>, f: F) -> Self {
                    Self { rows, map: f }
                }
            }

            impl<'stmt, T, F> Iterator for #MappedRows<'stmt, F>
            where
                F: FnMut(#outputs_declr) -> T
            {
                type Item = rusqlite::Result<T>;

                fn next(&mut self) -> Option<rusqlite::Result<T>> {
                    let map = &mut self.map;
                    self.rows
                        .next()
                        .transpose()
                        .map(|row_result| {
                            row_result.and_then(|row| {
                                #mapped_row_closure
                            })
                        })
                }
            }

            #[allow(non_camel_case_types)]
            pub struct #Rows<'stmt> {
                rows: rusqlite::Rows<'stmt>,
            }

            impl<'stmt> #Rows<'stmt> {
                pub(crate) fn new(rows: rusqlite::Rows<'stmt>) -> Self {
                    Self { rows }
                }
            }

            impl<'stmt> Iterator for #Rows<'stmt> {
                type Item = rusqlite::Result<(#outputs_declr)>;

                fn next(&mut self) -> Option<Self::Item> {
                    self.rows
                        .next()
                        .transpose()
                        .map(|row_result| {
                            row_result.and_then(|row| {
                                Ok((#row_closure))
                            })
                        })
                }
            }

            #[allow(non_camel_case_types)]
            pub struct #StatementType<'a>(pub rusqlite::Statement<'a>);

            impl<'a> #StatementType<'a> {
                fn query_map<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<#MappedRows<'_, F>>
                where
                    F: FnMut(#outputs_declr) -> T,
                {
                    let rows = self.0.query(#params_query)?;
                    Ok(#MappedRows::new(rows, f))
                }

                fn query_row<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<T>
                where
                    F: FnMut(#outputs_declr) -> T,
                {
                    let rows = self.query_map(#params_relay f)?;
                    for item in rows {
                        return Ok(item?);
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows)
                }

                fn query(&mut self #params_declr) -> rusqlite::Result<#Rows<'_>> {
                    let rows = self.0.query(#params_query)?;
                    Ok(#Rows::new(rows))
                }

                fn execute(&mut self #params_declr) -> rusqlite::Result<()> {
                    self.0.execute(#params_query)?;
                    Ok(())
                }
            }

            #[allow(non_camel_case_types)]
            pub struct #CachedStatementType<'a>(pub rusqlite::CachedStatement<'a>);

            impl<'a> #CachedStatementType<'a> {
                fn query_map<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<#MappedRows<'_, F>>
                where
                    F: FnMut(#outputs_declr) -> T,
                {
                    let rows = self.0.query(#params_query)?;
                    Ok(#MappedRows::new(rows, f))
                }

                fn query_row<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<T>
                where
                    F: FnMut(#outputs_declr) -> T,
                {
                    let rows = self.query_map(#params_relay f)?;
                    for item in rows {
                        return Ok(item?);
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows)
                }

                fn query(&mut self #params_declr) -> rusqlite::Result<#Rows<'_>> {
                    let rows = self.0.query(#params_query)?;
                    Ok(#Rows::new(rows))
                }

                fn execute(&mut self #params_declr) -> rusqlite::Result<()> {
                    self.0.execute(#params_query)?;
                    Ok(())
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
            Kind::Rusqlite => quote!{rusqlite::Connection},
            Kind::PostgreSQL => quote!{postgres::Client},
        };
        let client_ref_type = match self.kind {
            Kind::Rusqlite => quote!{&},
            Kind::PostgreSQL => quote!{&mut},
        };
        let ignore_error = match self.kind {
            Kind::Rusqlite => quote!{Err(rusqlite::Error::ExecuteReturnedResults) => {}},
            Kind::PostgreSQL => quote!{},
        };
        let error_type = match self.kind {
            Kind::Rusqlite => quote!{rusqlite::Error},
            Kind::PostgreSQL => quote!{postgres::Error},
        };
        let open_client = match self.kind {
            Kind::Rusqlite => quote!{
                let conn = #client_type::open_in_memory()?;
            },
            Kind::PostgreSQL => quote!{let mut conn = {
                let mut conn = Client::connect("user=postgres host=localhost port=5433", NoTls).unwrap();
                conn.execute("SET search_path TO pg_temp", &[]).unwrap();
                conn
            }; },
        };

        let test = if let Some(depends) = &self.test {
            let depends = depends.iter().map(|name| {
                let parent_testsetup_name =
                    Ident::new(&format!("testsetup_{}", name), self.name.span());
                quote! {
                    #parent_testsetup_name(uns, deps, conn)?;
                }
            });
            quote! {
                #[cfg(test)]
                fn #testsetup_name(
                    uns: &mut arbitrary::Unstructured,
                    deps: &mut std::collections::HashSet<&'static str>,
                    conn: #client_ref_type #client_type) -> Result<(), #error_type>
                {
                    if !deps.insert(#name) {
                        return Ok(());
                    }

                    #(#depends);*

                    #params_arbit_prep;
                    let r = conn.#execute_name(#params_arbit);
                    match r {
                        Ok(_) => {}
                        #ignore_error
                        Err(err) => {
                            eprintln!("{:?}", err);
                            Err(err)?;
                        },
                    }
                    Ok(())
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
        let specifier = syn::LitStr::new(&format!(":{}", name), name.span());

        match query.kind {
            Kind::Rusqlite => quote! { (#specifier, &#name as &dyn rusqlite::ToSql) },
            Kind::PostgreSQL => quote! { &#name as &(dyn postgres::types::ToSql + Sync) }
        }
    }
}

enum Attr {
    Kind(Kind),
    Test(Vec<TestAttr>),
    Named,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident == "rusqlite" {
            return Ok(Attr::Kind(Kind::Rusqlite));
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

/// The general structure of the input to the `fnsql` macro is the following:
///
/// ```ignore
/// fnsql! {
///     #[<sql-engine-type>, [OPTIONAL: test(with=[other-function-a, other-function-b...])]]
///     <function-name-a>(param1: type, param2: type...)
///          [OPTIONAL: -> [(col a type, col b type, ...)]]
///     {
///         "SQL QUERY STRING"
///     }
///
///     ...
/// }
/// ```
///
/// **For examples see the root doc of the `fnsql` crate.**
///
/// - Return type is optional, and only meaningful for SQL operations that return row data.
/// - The only supported `sgl-engine-type` is `rusqlite`.
/// - Testing is optional - you have to specific the `test` attribute for it.
/// - With `test(with=[...])`, you specify the quries that need execution for this
///   query to work.
///
#[proc_macro]
pub fn fnsql(input: TokenStream) -> TokenStream {
    let queries: Queries = parse_macro_input!(input);
    let queries: Vec<_> = queries.list.iter().map(|x| x.expand()).collect();

    quote! { #(#queries)* }.into()
}
