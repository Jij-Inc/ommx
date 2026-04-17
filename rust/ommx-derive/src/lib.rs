//! Derive macros for the `ommx` crate.
//!
//! This crate is for internal use within the OMMX workspace only.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Derive `LogicalMemoryProfile` for a struct by delegating to each field.
///
/// Only structs with named fields are supported. Each field's profile is
/// emitted under the path frame `"TypeName.field_name"`.
#[proc_macro_derive(LogicalMemoryProfile)]
pub fn derive_logical_memory_profile(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
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
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                name,
                "LogicalMemoryProfile derive only supports structs",
            )
            .to_compile_error()
            .into();
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

    let expanded = quote! {
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
    };

    TokenStream::from(expanded)
}
