//! Derive macros for the `ommx` crate.
//!
//! This crate exists solely as an implementation detail of [`ommx`]: it
//! is published to crates.io because `ommx` depends on it, but it has no
//! stable API of its own and **no public surface for external use**.
//!
//! The `ommx` crate gates the `LogicalMemoryProfile` trait and the
//! re-exported derive behind a `pub(crate)` module, so downstream users
//! cannot reach them through the public API and cannot meaningfully
//! derive on their own types. The trait and the re-export are declared
//! `pub` *inside* that module to satisfy the `private_bounds` lint when
//! the trait appears in the bound of a `pub` type within the crate
//! (e.g. `ConstraintMetadataStore<ID: ... + LogicalMemoryProfile>`).
//! External consumers should use
//! [`ommx::Instance::logical_memory_profile`] and
//! [`ommx::MemoryProfile`] instead.
//!
//! [`ommx`]: https://docs.rs/ommx
//! [`ommx::Instance::logical_memory_profile`]: https://docs.rs/ommx/latest/ommx/struct.Instance.html#method.logical_memory_profile
//! [`ommx::MemoryProfile`]: https://docs.rs/ommx/latest/ommx/struct.MemoryProfile.html
//!
//! # `#[derive(LogicalMemoryProfile)]`
//!
//! Generates a `LogicalMemoryProfile` impl by following the type structure.
//! Named-field structs delegate to each field under `"TypeName.field_name"`;
//! tuple structs delegate under `"TypeName.0"`, `"TypeName.1"`, and so on.
//! Unit structs emit no leaves. Fieldless enums emit one inline leaf of
//! `std::mem::size_of::<Self>()`.
//!
//! ## Supported
//!
//! - Structs with named fields, tuple fields, or no fields.
//!   - All fields must implement `LogicalMemoryProfile`. Primitives,
//!     `String`, `Option<T>`, arrays, `Box<T>`, `Vec<T>`,
//!     `VecDeque<T>`, maps, sets, `PhantomData<T>`, and tuples all have
//!     reusable impls in `ommx::logical_memory::collections`.
//! - Generic structs: when a field type depends on a type parameter, the
//!   generated impl adds a `FieldType: LogicalMemoryProfile` where-clause.
//!   This keeps composite structs derivable without hand-written impls.
//! - Fieldless enums. Data-carrying enums should keep a hand-written impl
//!   until variant-aware enum decomposition is introduced.
//!
//! ## Not supported
//!
//! - Data-carrying enum variants → emit a `compile_error!`.
//! - Field skipping → there is no `#[logical_memory(skip)]` attribute.
//!   Composite structs should not skip fields; extend this derive if a
//!   composite profiling use case needs more macro support.
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
use syn::{parse_quote, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

/// Derive `LogicalMemoryProfile` for a type.
///
/// Struct fields are delegated structurally. Fieldless enums emit one
/// `size_of::<Self>()` leaf.
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

    let mut generics = input.generics.clone();
    let body = match &input.data {
        Data::Struct(data) => {
            let field_visits = struct_field_visits(name, &data.fields);
            if has_type_params(&generics) {
                for field in data.fields.iter() {
                    if type_uses_type_param(&field.ty, &generics) {
                        let ty = &field.ty;
                        generics
                            .make_where_clause()
                            .predicates
                            .push(parse_quote!(#ty: ::ommx::logical_memory::LogicalMemoryProfile));
                    }
                }
            }
            quote! {
                #( #field_visits )*
            }
        }
        Data::Enum(data) => {
            if let Some(variant) = data
                .variants
                .iter()
                .find(|variant| !matches!(variant.fields, Fields::Unit))
            {
                return syn::Error::new_spanned(
                    variant,
                    "LogicalMemoryProfile derive only supports fieldless enums",
                )
                .to_compile_error();
            }
            quote! {
                visitor.visit_leaf(path, ::std::mem::size_of::<Self>());
            }
        }
        _ => {
            return syn::Error::new_spanned(
                name,
                "LogicalMemoryProfile derive only supports structs and fieldless enums",
            )
            .to_compile_error();
        }
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics ::ommx::logical_memory::LogicalMemoryProfile
            for #name #ty_generics #where_clause
        {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                #body
            }
        }
    }
}

fn struct_field_visits(name: &syn::Ident, fields: &Fields) -> Vec<TokenStream2> {
    let name_str = name.to_string();
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let access = field
                .ident
                .as_ref()
                .map(|ident| quote!(#ident))
                .unwrap_or_else(|| {
                    let index = syn::Index::from(index);
                    quote!(#index)
                });
            let field_name = field
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| index.to_string());
            let frame = format!("{name_str}.{field_name}");
            quote! {
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.#access,
                    path.with(#frame).as_mut(),
                    visitor,
                );
            }
        })
        .collect()
}

fn has_type_params(generics: &syn::Generics) -> bool {
    generics.type_params().next().is_some()
}

fn type_uses_type_param(ty: &Type, generics: &syn::Generics) -> bool {
    match ty {
        Type::Array(ty) => type_uses_type_param(&ty.elem, generics),
        Type::FnPtr(ty) => {
            ty.inputs
                .iter()
                .any(|arg| type_uses_type_param(&arg.ty, generics))
                || matches!(
                    &ty.output,
                    syn::ReturnType::Type(_, output) if type_uses_type_param(output, generics)
                )
        }
        Type::Group(ty) => type_uses_type_param(&ty.elem, generics),
        Type::Paren(ty) => type_uses_type_param(&ty.elem, generics),
        Type::Path(ty) => {
            ty.qself
                .as_ref()
                .is_some_and(|qself| type_uses_type_param(&qself.ty, generics))
                || ty.path.segments.iter().any(|segment| {
                    generics
                        .type_params()
                        .any(|param| param.ident == segment.ident)
                        || path_arguments_use_type_param(&segment.arguments, generics)
                })
        }
        Type::Ptr(ty) => type_uses_type_param(&ty.elem, generics),
        Type::Reference(ty) => type_uses_type_param(&ty.elem, generics),
        Type::Slice(ty) => type_uses_type_param(&ty.elem, generics),
        Type::Tuple(ty) => ty
            .elems
            .iter()
            .any(|elem| type_uses_type_param(elem, generics)),
        _ => false,
    }
}

fn path_arguments_use_type_param(arguments: &PathArguments, generics: &syn::Generics) -> bool {
    match arguments {
        PathArguments::None => false,
        PathArguments::AngleBracketed(arguments) => {
            arguments.args.iter().any(|argument| match argument {
                GenericArgument::Type(ty) => type_uses_type_param(ty, generics),
                GenericArgument::AssocType(assoc) => type_uses_type_param(&assoc.ty, generics),
                GenericArgument::Constraint(constraint) => constraint.bounds.iter().any(|bound| {
                    matches!(
                        bound,
                        syn::TypeParamBound::Trait(bound)
                            if bound.path.segments.iter().any(|segment| {
                                path_arguments_use_type_param(&segment.arguments, generics)
                            })
                    )
                }),
                _ => false,
            })
        }
        PathArguments::Parenthesized(arguments) => {
            arguments
                .inputs
                .iter()
                .any(|input| type_uses_type_param(&input.ty, generics))
                || matches!(
                    &arguments.output,
                    syn::ReturnType::Type(_, output) if type_uses_type_param(output, generics)
                )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_uses_param(ty: Type) -> bool {
        let generics: syn::Generics = syn::parse_quote!(<T>);
        type_uses_type_param(&ty, &generics)
    }

    #[test]
    fn detects_type_param_in_function_pointer() {
        assert!(type_uses_param(syn::parse_quote!(fn(T) -> u64)));
        assert!(type_uses_param(syn::parse_quote!(fn(u64) -> T)));
        assert!(!type_uses_param(syn::parse_quote!(fn(u64) -> bool)));
    }

    #[test]
    fn detects_type_param_in_parenthesized_path_arguments() {
        let generics: syn::Generics = syn::parse_quote!(<T>);
        for (bound, expected) in [
            (syn::parse_quote!(Fn(T) -> u64), true),
            (syn::parse_quote!(Fn(u64) -> T), true),
            (syn::parse_quote!(Fn(u64) -> bool), false),
        ] {
            let syn::TypeParamBound::Trait(bound) = bound else {
                panic!("expected trait bound");
            };
            assert_eq!(
                path_arguments_use_type_param(&bound.path.segments[0].arguments, &generics),
                expected
            );
        }
    }

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
        let input = quote! {
            struct Generic<T> {
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
    fn snapshot_generic_field_type_bound() {
        let input = quote! {
            struct GenericMap<K, V> {
                entries: std::collections::BTreeMap<K, V>,
                label: String,
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        impl<K, V> ::ommx::logical_memory::LogicalMemoryProfile for GenericMap<K, V>
        where
            std::collections::BTreeMap<K, V>: ::ommx::logical_memory::LogicalMemoryProfile,
        {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.entries,
                    path.with("GenericMap.entries").as_mut(),
                    visitor,
                );
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.label,
                    path.with("GenericMap.label").as_mut(),
                    visitor,
                );
            }
        }
        "###);
    }

    #[test]
    fn snapshot_fieldless_enum() {
        let input = quote! {
            enum Equality {
                EqualToZero,
                LessThanOrEqualToZero,
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        impl ::ommx::logical_memory::LogicalMemoryProfile for Equality {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                visitor.visit_leaf(path, ::std::mem::size_of::<Self>());
            }
        }
        "###);
    }

    #[test]
    fn snapshot_tuple_struct() {
        let input = quote! {
            struct Tuple(u64, String);
        };
        insta::assert_snapshot!(render(input), @r###"
        impl ::ommx::logical_memory::LogicalMemoryProfile for Tuple {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.0,
                    path.with("Tuple.0").as_mut(),
                    visitor,
                );
                ::ommx::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    &self.1,
                    path.with("Tuple.1").as_mut(),
                    visitor,
                );
            }
        }
        "###);
    }

    #[test]
    fn snapshot_unit_struct() {
        let input = quote! {
            struct Unit;
        };
        insta::assert_snapshot!(render(input), @r###"
        impl ::ommx::logical_memory::LogicalMemoryProfile for Unit {
            fn visit_logical_memory<__V: ::ommx::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut ::ommx::logical_memory::Path,
                visitor: &mut __V,
            ) {}
        }
        "###);
    }

    #[test]
    fn snapshot_rejects_data_enum() {
        let input = quote! {
            enum Provenance {
                IndicatorConstraint(IndicatorConstraintID),
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        ::core::compile_error! {
            "LogicalMemoryProfile derive only supports fieldless enums"
        }
        "###);
    }

    #[test]
    fn snapshot_rejects_union() {
        let input = quote! {
            union Foo {
                value: u64,
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        ::core::compile_error! {
            "LogicalMemoryProfile derive only supports structs and fieldless enums"
        }
        "###);
    }
}
