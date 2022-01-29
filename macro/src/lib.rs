//! The `fnsql` crate provides simple type-safe optional wrappers around SQL
//! queries. Instead of calling type-less `.query()` and `.execute()`, you call to
//! auto-generated unique wrappers that are strongly typed, `.query_<name>()` and
//! `.execute_<name>()`. However, you manually specify the input and output types,
//! but only once, with the query, and in separation with the code that uses the
//! query.
//!
//! It's a very simple implementation that doesn't force any schema or ORM down
//! your throat, so if you are already using `rusqlite`, you can't gradually
//! replace your type-less queries with the type-ful wrappers.
//!
//! The way to generate these wrappers is to specify input and output types for
//! each of the queries, for example, consider the following definitions
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
//!     get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>)] {
//!         "SELECT id, data FROM pet WHERE pet.name = :name"
//!     }
//!
//!     #[rusqlite, test(with=[create_table_pet])]
//!     insert_new_pet(name: String, data: Option<Vec<u8>>) {
//!         "INSERT INTO pet (name, data) VALUES (:name, :data)"
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
//! // let mut stmt = conn.prepare("SELECT id, name, data FROM pet")?;
//!
//! let pet_iter = stmt.query(&Some("Max".to_string()), |_id, data| {
//!     Ok::<_, rusqlite::Error>(Pet {
//!         _id,
//!         data,
//!         name: "Max".to_string(),
//!     })
//! })?;
//! // let pet_iter = stmt.query_map([], |row| {
//! //     Ok(Person {
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
//!  * Does not do any compile-time validation based on SQL query string.
//!  * Only supports rusqlite

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Token,
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
}

struct Query {
    name: syn::Ident,
    params: Vec<Param>,
    outputs: Vec<Output>,
    query: syn::LitStr,
    kind: Kind,
    test: Option<Vec<String>>,
}

impl Parse for Query {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut kind = None;
        let mut test = None;

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
        })
    }
}

impl Query {
    fn prepend_name(&self, prefix: &'static str) -> syn::Ident {
        syn::Ident::new(&format!("{}{}", prefix, &self.name), self.name.span())
    }

    fn params_declr(&self) -> Tokens {
        let list: Vec<_> = self.params.iter().map(|x| x.expand_declr()).collect();
        quote! { #(, #list)* }
    }

    fn outputs_declr(&self) -> Tokens {
        let list: Vec<_> = self.outputs.iter().map(|x| x.expand_declr()).collect();
        quote! { #(#list),* }
    }

    fn outputs_row_closure(&self) -> Tokens {
        let list: Vec<_> = self
            .outputs
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let i = syn::LitInt::new(&format!("{}", i), self.name.span());
                quote! {row.get(#i)?}
            })
            .collect();

        quote! { Ok(map(#(#list),*)) }
    }

    fn params_arbitrary(&self) -> Tokens {
        let list: Vec<_> = self
            .params
            .iter()
            .map(|_| {
                quote! {&arbitrary::Arbitrary::arbitrary(uns).unwrap()}
            })
            .collect();

        quote! { #(#list),* }
    }

    fn params_query(&self) -> Tokens {
        let list: Vec<_> = self.params.iter().map(|x| x.expand_query()).collect();
        if list.len() == 0 {
            quote! { [] }
        } else {
            quote! { &[#(#list),*] }
        }
    }

    fn expand(&self) -> Tokens {
        let conn_trait_name = self.prepend_name("Connection_");
        let test_name = self.prepend_name("auto_");
        let testsetup_name = self.prepend_name("testsetup_");
        #[allow(non_snake_case)]
        let StatementType = self.prepend_name("Statement_");
        #[allow(non_snake_case)]
        let CachedStatementType = self.prepend_name("CachedStatement_");
        let rows_struct_name = self.prepend_name("Rows_");
        let prepare_name = self.prepend_name("prepare_");
        let prepare_cached_name = self.prepend_name("prepare_cached_");
        let execute_name = self.prepend_name("execute_");
        let params_declr = self.params_declr();
        let outputs_declr = self.outputs_declr();
        let params_arbit = self.params_arbitrary();
        let row_closure = self.outputs_row_closure();
        let params_query = self.params_query();
        let query = &self.query;
        let name = syn::LitStr::new(&self.name.to_string(), self.name.span());

        let test = if let Some(depends) = &self.test {
            let depends = depends.iter().map(|name| {
                let parent_testsetup_name =
                    syn::Ident::new(&format!("testsetup_{}", name), self.name.span());
                quote! {
                    #parent_testsetup_name(uns, deps, conn)?;
                }
            });
            quote! {
                #[cfg(test)]
                fn #testsetup_name(
                    uns: &mut arbitrary::Unstructured,
                    deps: &mut std::collections::HashSet<&'static str>,
                    conn: &rusqlite::Connection) -> rusqlite::Result<()>
                {
                    if !deps.insert(#name) {
                        return Ok(());
                    }

                    #(#depends);*

                    conn.#execute_name(#params_arbit)?;
                    Ok(())
                }

                #[test]
                fn #test_name() -> rusqlite::Result<()> {
                    let conn = rusqlite::Connection::open_in_memory()?;
                    let mut deps = std::collections::HashSet::new();
                    let raw_data: &[u8] = &[1, 2, 3];
                    let mut unstructured = arbitrary::Unstructured::new(raw_data);

                    #testsetup_name(&mut unstructured, &mut deps, &conn)?;
                    Ok(())
                }
            }
        } else {
            quote! {}
        };

        matches!(self.kind, Kind::Rusqlite);

        quote! {
            #[allow(non_camel_case_types)]
            trait #conn_trait_name {
                fn #prepare_name(&self) -> rusqlite::Result<#StatementType<'_>>;
                fn #prepare_cached_name(&self) -> rusqlite::Result<#CachedStatementType<'_>>;
                fn #execute_name(&self #params_declr) -> rusqlite::Result<usize>;
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
            }

            #[allow(non_camel_case_types)]
            pub struct #rows_struct_name<'stmt, F> {
                rows: rusqlite::Rows<'stmt>,
                map: F,
            }

            impl<'stmt, T, F> #rows_struct_name<'stmt, F>
            where
                F: FnMut(#outputs_declr) -> T
            {
                pub(crate) fn new(rows: rusqlite::Rows<'stmt>, f: F) -> Self {
                    Self { rows, map: f }
                }
            }

            impl<'stmt, T, F> Iterator for #rows_struct_name<'stmt, F>
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
                                #row_closure
                            })
                        })
                }
            }

            #[allow(non_camel_case_types)]
            struct #StatementType<'a>(pub rusqlite::Statement<'a>);

            impl<'a> #StatementType<'a> {
                fn query<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<#rows_struct_name<'_, F>>
                where
                    F: FnMut(#outputs_declr) -> T,
                {
                    let rows = self.0.query(#params_query)?;
                    Ok(#rows_struct_name::new(rows, f))
                }
            }

            #[allow(non_camel_case_types)]
            struct #CachedStatementType<'a>(pub rusqlite::CachedStatement<'a>);

            impl<'a> #CachedStatementType<'a> {
                fn query<F, T>(&mut self #params_declr, f: F) -> rusqlite::Result<#rows_struct_name<'_, F>>
                where
                    F: FnMut(#outputs_declr) -> T,
                {
                    let rows = self.0.query(#params_query)?;
                    Ok(#rows_struct_name::new(rows, f))
                }
            }

            #test
        }
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
    name: syn::Ident,
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

    fn expand_query(&self) -> Tokens {
        let name = &self.name;
        let specifier = syn::LitStr::new(&format!(":{}", name), name.span());

        quote! { (#specifier, &#name as &dyn rusqlite::ToSql) }
    }
}

enum Attr {
    Kind(Kind),
    Test(Vec<TestAttr>),
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        if ident == "rusqlite" {
            return Ok(Attr::Kind(Kind::Rusqlite));
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
        let ident: syn::Ident = input.parse()?;
        if ident == "with" {
            let mut v = vec![];

            let _: Token![=] = input.parse()?;
            let content;
            let _ = bracketed!(content in input);
            let list: Punctuated<syn::Ident, Token![,]> = content.parse_terminated(Parse::parse)?;
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
