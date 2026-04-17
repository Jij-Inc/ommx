//! Derive macros for the `ommx` crate.
//!
//! This crate is for internal use within the OMMX workspace only.

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
