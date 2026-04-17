//! Derive macros for the `ommx` crate.
//!
//! This crate exists solely as an implementation detail of [`ommx`]: it
//! is published to crates.io because `ommx` depends on it, but it has no
//! stable API of its own. Depend on `ommx` and use the re-exported derive
//! rather than pulling this crate in directly.
//!
//! [`ommx`]: https://docs.rs/ommx
//!
//! # `#[derive(LogicalMemoryProfile)]`
//!
//! Generates a `LogicalMemoryProfile` impl that delegates to each field
//! of a named-field struct. Each field is emitted under the path frame
//! `"TypeName.field_name"`. The `ommx` crate uses this derive at every
//! struct definition that participates in memory profiling, so that
//! adding or removing a field automatically adjusts the profile.
//!
//! ## Supported
//!
//! - Structs with named fields.
//!   - All fields must implement `LogicalMemoryProfile`. Primitives,
//!     `String`, `Option<T>`, `Vec<T>`, `BTreeMap`, `HashMap`,
//!     `FnvHashMap`, and `BTreeSet` all have blanket impls in
//!     `ommx::logical_memory::collections`.
//! - Generic structs: type parameters are propagated through, but
//!   **no `LogicalMemoryProfile` bound is added automatically**. The
//!   struct must declare its own `where T: LogicalMemoryProfile`
//!   clause. This matches `serde`'s historical `#[serde(bound = ...)]`
//!   philosophy — the derive does not guess.
//!
//! ## Not supported
//!
//! - Tuple structs and unit structs → emit a `compile_error!` directing
//!   the caller to a hand-written impl.
//! - Enums → emit a `compile_error!`. For enums, hand-write a `match`
//!   (`Function` in the `ommx` crate is an example).
//! - Field skipping → there is no `#[logical_memory(skip)]` attribute.
//!   If a field truly should not participate, hand-write the impl.
//! - Custom frame names → the frame is always `"TypeName.field_name"`
//!   taken from the struct ident and field ident. For a renamed frame
//!   (e.g. when wrapping an external type), use the declarative
//!   `impl_logical_memory_profile! { path::to::Type as "Name" { ... } }`
//!   form instead.
//!
//! # Testing
//!
//! The proc-macro entry point delegates to
//! `derive_logical_memory_profile_impl`, a pure
//! `TokenStream2 -> TokenStream2` function. This is exercised by inline
//! `insta` snapshot tests in this crate — the generated code is checked
//! in as a snapshot so any drift is caught at review time.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

/// Derive `LogicalMemoryProfile` for a struct by delegating to each field.
///
/// Only structs with named fields are supported. Each field's profile is
/// emitted under the path frame `"TypeName.field_name"`.
#[proc_macro_derive(LogicalMemoryProfile)]
pub fn derive_logical_memory_profile(input: TokenStream) -> TokenStream {
    derive_logical_memory_profile_impl(input.into()).into()
}

/// Pure `TokenStream2` entry point for the derive.
///
/// Split out from the `#[proc_macro_derive]` wrapper so that unit tests
/// can exercise the code-generation logic without the proc-macro runtime.
fn derive_logical_memory_profile_impl(input: TokenStream2) -> TokenStream2 {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(ast) => ast,
        Err(err) => return err.to_compile_error(),
    };
    let name = &input.ident;
    let name_str = name.to_string();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "LogicalMemoryProfile derive only supports structs with named fields",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                name,
                "LogicalMemoryProfile derive only supports structs",
            )
            .to_compile_error();
        }
    };

    let field_visits = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().expect("named field");
        let frame = format!("{name_str}.{field_name}");
        quote! {
            ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                &self.#field_name,
                path.with(#frame).as_mut(),
                visitor,
            );
        }
    });

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics ::ommx::logical_memory::LogicalMemoryProfile
            for #name #ty_generics #where_clause
        {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                #( #field_visits )*
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render a derive-generated `TokenStream2` as a formatted Rust source
    /// string, so `insta::assert_snapshot!` diffs are readable.
    fn render(input: TokenStream2) -> String {
        let tokens = derive_logical_memory_profile_impl(input);
        let file: syn::File = syn::parse2(tokens).expect("derive output must parse as syn::File");
        prettyplease::unparse(&file)
    }

    #[test]
    fn snapshot_flat_struct() {
        let input = quote! {
            struct Foo {
                a: u64,
                b: String,
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        impl ::ommx::logical_memory::LogicalMemoryProfile for Foo {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.a,
                    path.with("Foo.a").as_mut(),
                    visitor,
                );
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.b,
                    path.with("Foo.b").as_mut(),
                    visitor,
                );
            }
        }
        "###);
    }

    #[test]
    fn snapshot_single_field_struct() {
        let input = quote! {
            struct Wrapper {
                inner: Inner,
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        impl ::ommx::logical_memory::LogicalMemoryProfile for Wrapper {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.inner,
                    path.with("Wrapper.inner").as_mut(),
                    visitor,
                );
            }
        }
        "###);
    }

    #[test]
    fn snapshot_empty_struct() {
        // Unit-like structs with empty named-field bodies are legal; the
        // derive should emit an empty `visit_logical_memory` body.
        let input = quote! {
            struct Empty {}
        };
        insta::assert_snapshot!(render(input), @r###"
        impl ::ommx::logical_memory::LogicalMemoryProfile for Empty {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {}
        }
        "###);
    }

    #[test]
    fn snapshot_generic_struct() {
        // Generic parameters are propagated without automatic trait-bound
        // injection; callers must ensure `T: LogicalMemoryProfile` themselves
        // (e.g. via a `where` clause on the struct definition).
        let input = quote! {
            struct Generic<T> where T: ::ommx::logical_memory::LogicalMemoryProfile {
                value: T,
                count: u64,
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        impl<T> ::ommx::logical_memory::LogicalMemoryProfile for Generic<T>
        where
            T: ::ommx::logical_memory::LogicalMemoryProfile,
        {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.value,
                    path.with("Generic.value").as_mut(),
                    visitor,
                );
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.count,
                    path.with("Generic.count").as_mut(),
                    visitor,
                );
            }
        }
        "###);
    }

    #[test]
    fn snapshot_rejects_enum() {
        // Error output is also snapshot-tested to lock in the diagnostic
        // message surface. The generated compile_error! invocation is the
        // contract for non-struct inputs.
        let input = quote! {
            enum NotSupported { A, B }
        };
        insta::assert_snapshot!(render(input), @r###"
        ::core::compile_error! {
            "LogicalMemoryProfile derive only supports structs"
        }
        "###);
    }

    #[test]
    fn snapshot_rejects_tuple_struct() {
        let input = quote! {
            struct Tuple(u64, String);
        };
        insta::assert_snapshot!(render(input), @r###"
        ::core::compile_error! {
            "LogicalMemoryProfile derive only supports structs with named fields"
        }
        "###);
    }
}
