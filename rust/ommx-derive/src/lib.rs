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
//! Generates a `LogicalMemoryProfile` impl. By default this delegates to
//! each field of a named-field struct, with each field emitted under the
//! path frame `"TypeName.field_name"`. The `ommx` crate uses this derive
//! at every struct definition that participates in memory profiling, so
//! that adding or removing a field automatically adjusts the profile.
//!
//! Leaf-like types can instead opt in to `#[logical_memory(leaf)]`, which
//! emits a single leaf of `std::mem::size_of::<Self>()` at the current path.
//! This is intended for POD structs, tuple newtypes, and small enums whose
//! logical memory is their inline representation.
//!
//! ## Supported
//!
//! - Structs with named fields.
//!   - All fields must implement `LogicalMemoryProfile`. Primitives,
//!     `String`, `Option<T>`, `Vec<T>`, `BTreeMap`, `HashMap`,
//!     `FnvHashMap`, and `BTreeSet` all have blanket impls in
//!     `ommx::logical_memory::collections`.
//! - Generic structs: when a field type depends on a type parameter, the
//!   generated impl adds a `FieldType: LogicalMemoryProfile` where-clause.
//!   This keeps composite structs derivable without hand-written impls.
//! - `#[logical_memory(leaf)]` on structs, tuple structs, unit structs, and
//!   enums. Leaf mode does not add field bounds because it does not inspect
//!   fields.
//!
//! ## Not supported
//!
//! - Tuple structs and unit structs without `#[logical_memory(leaf)]` → emit
//!   a `compile_error!`.
//! - Enums without `#[logical_memory(leaf)]` → emit a `compile_error!`.
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
/// By default, only structs with named fields are supported, and each field's
/// profile is emitted under the path frame `"TypeName.field_name"`. With
/// `#[logical_memory(leaf)]`, the type emits one `size_of::<Self>()` leaf.
#[proc_macro_derive(LogicalMemoryProfile, attributes(logical_memory))]
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

    let attrs = match parse_logical_memory_attrs(&input.attrs) {
        Ok(attrs) => attrs,
        Err(err) => return err.to_compile_error(),
    };
    if attrs.leaf {
        return derive_leaf_impl(&input);
    }

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

    let mut generics = input.generics.clone();
    if has_type_params(&generics) {
        for field in fields {
            if type_uses_type_param(&field.ty, &generics) {
                let ty = &field.ty;
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: ::ommx::logical_memory::LogicalMemoryProfile));
            }
        }
    }
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
                #( #field_visits )*
            }
        }
    }
}

#[derive(Default)]
struct LogicalMemoryAttrs {
    leaf: bool,
}

fn parse_logical_memory_attrs(attrs: &[syn::Attribute]) -> syn::Result<LogicalMemoryAttrs> {
    let mut parsed = LogicalMemoryAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("logical_memory") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("leaf") {
                if parsed.leaf {
                    return Err(meta.error("duplicate `leaf` logical_memory attribute"));
                }
                parsed.leaf = true;
                Ok(())
            } else {
                Err(meta.error("unsupported logical_memory attribute; expected `leaf`"))
            }
        })?;
    }

    Ok(parsed)
}

fn derive_leaf_impl(input: &DeriveInput) -> TokenStream2 {
    let name = &input.ident;
    let generics = input.generics.clone();
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
                visitor.visit_leaf(path, ::std::mem::size_of::<Self>());
            }
        }
    }
}

fn has_type_params(generics: &syn::Generics) -> bool {
    generics.type_params().next().is_some()
}

fn type_uses_type_param(ty: &Type, generics: &syn::Generics) -> bool {
    match ty {
        Type::Array(ty) => type_uses_type_param(&ty.elem, generics),
        Type::BareFn(ty) => {
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
                .any(|input| type_uses_type_param(input, generics))
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

    #[test]
    fn snapshot_leaf_tuple_struct() {
        let input = quote! {
            #[logical_memory(leaf)]
            struct VariableID(u64);
        };
        insta::assert_snapshot!(render(input), @r###"
        impl ::ommx::logical_memory::LogicalMemoryProfile for VariableID {
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
    fn snapshot_leaf_enum() {
        let input = quote! {
            #[logical_memory(leaf)]
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
    fn snapshot_rejects_unknown_attribute() {
        let input = quote! {
            #[logical_memory(skip)]
            struct Foo {
                value: u64,
            }
        };
        insta::assert_snapshot!(render(input), @r###"
        ::core::compile_error! {
            "unsupported logical_memory attribute; expected `leaf`"
        }
        "###);
    }
}
