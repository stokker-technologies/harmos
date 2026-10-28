/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Error, Fields, ItemEnum, Meta, Token};
// Local Imports
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn declare(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, item, true, false, false, true) {
        Ok(expanded) => expanded,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn declare_runtime(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, item, true, true, false, true) {
        Ok(expanded) => expanded,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn declare_domain(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, item, true, false, true, true) {
        Ok(expanded) => expanded,
        Err(error) => error.to_compile_error(),
    }
}

/* === Private Functions === */

pub(super) fn projection(attribute: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    expand(attribute, item, false, false, false, true)
}

fn expand(
    attribute: TokenStream, item: TokenStream, operations: bool, runtime: bool, domain: bool, emit_declaration: bool,
) -> Result<TokenStream, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut explicit = None;
    for argument in arguments {
        let Some(value) = path::declared(&argument) else {
            return Err(Error::new_spanned(argument, "catalog accepts only harmos_path = ..."));
        };
        if explicit.replace(value?).is_some() {
            return Err(Error::new_spanned(argument, "harmos_path may be declared only once"));
        }
    }
    let mut sdk = explicit.map(|path| HarmosPath::resolve(Some(path))).unwrap_or_else(HarmosPath::catalog);
    if runtime {
        sdk = sdk.runtime_enabled();
    }

    let mut declared: ItemEnum = syn::parse2(item)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut variants = std::collections::BTreeSet::new();
    let mut members = Vec::new();
    let mut helpers = Vec::new();
    let mut helper_types = Vec::new();
    let mut direct = Vec::new();
    let catalog_name = declared.ident.clone();
    let catalog_generics = declared.generics.clone();
    let gates = crate::operation::configuration(&declared.attrs)?;
    for variant in &mut declared.variants {
        if !variants.insert(variant.ident.to_string()) {
            return Err(Error::new_spanned(&variant.ident, "a catalog variant name may appear only once"));
        }
        if variant.discriminant.is_some() {
            return Err(Error::new_spanned(variant, "catalog variants cannot have discriminants"));
        }
        if variant.attrs.iter().any(|attribute| attribute.path().is_ident("operation")) {
            return Err(Error::new_spanned(variant, "declare operations on structs in vertical files; catalogs only assemble types"));
        }
        let Fields::Unnamed(fields) = &variant.fields else {
            return Err(Error::new_spanned(variant, "a catalog variant has exactly one unnamed typed field"));
        };
        if fields.unnamed.len() != 1 {
            return Err(Error::new_spanned(fields, "a catalog variant has exactly one unnamed typed field"));
        }
        let field = &fields.unnamed[0];
        if !field.attrs.is_empty() {
            return Err(Error::new_spanned(field, "catalog member fields cannot carry attributes"));
        }
        let member = &field.ty;
        if !seen.insert(member.to_token_stream().to_string()) {
            return Err(Error::new_spanned(member, "a catalog member type may appear only once"));
        }
        let mut nested = false;
        for attribute in &variant.attrs {
            if attribute.path().is_ident("nested") {
                if nested || !matches!(attribute.meta, Meta::Path(_)) {
                    return Err(Error::new_spanned(attribute, "declare #[nested] exactly once without arguments"));
                }
                nested = true;
            }
        }
        let member_gates = crate::operation::configuration(&variant.attrs)?;
        if !nested {
            direct.push((member.clone(), member_gates.clone()));
        }
        if member_gates.is_empty() {
            members.push((member.clone(), nested));
        } else {
            let helper = quote::format_ident!("__Harmos{}{}", catalog_name, variant.ident);
            let predicate = conditions(&member_gates)?;
            let nesting = nested.then(|| quote!(#[nested]));
            let enabled = quote! {
                #[cfg(#predicate)]
                pub enum #helper #catalog_generics { #nesting Member(#member) }
            };
            let disabled = quote! {
                #[cfg(not(#predicate))]
                pub enum #helper #catalog_generics {}
            };
            helper_types.push(quote!(#(#gates)* #enabled #(#gates)* #disabled));
            helpers.push(expand(quote!(harmos_path = #sdk), enabled, operations, runtime, domain, false)?);
            helpers.push(expand(quote!(harmos_path = #sdk), disabled, operations, runtime, domain, false)?);
            let (_, arguments, _) = catalog_generics.split_for_impl();
            members.push((syn::parse_quote!(#helper #arguments), true));
        }
        variant.attrs.retain(|attribute| !attribute.path().is_ident("nested"));
    }

    let count = members.len();
    let name = &declared.ident;
    let mut visitor_name = "__HarmosVisitor".to_owned();
    while declared.generics.params.iter().any(|parameter| match parameter {
        syn::GenericParam::Type(parameter) => parameter.ident == visitor_name,
        syn::GenericParam::Const(parameter) => parameter.ident == visitor_name,
        syn::GenericParam::Lifetime(_) => false,
    }) {
        visitor_name.push('_');
    }
    let visitor = syn::Ident::new(&visitor_name, proc_macro2::Span::mixed_site());
    let mut visitor_generics = declared.generics.clone();
    visitor_generics.params.push(syn::parse_quote!(#visitor));
    visitor_generics.make_where_clause().predicates.push(syn::parse_quote!(#visitor: #sdk::catalog::Visitor));
    for (member, nested) in &members {
        let bound = if *nested {
            syn::parse_quote!(#member: #sdk::catalog::Catalog<#visitor>)
        } else {
            syn::parse_quote!(#visitor: #sdk::catalog::Member<#member>)
        };
        visitor_generics.make_where_clause().predicates.push(bound);
    }
    let (entries_type, entries) = if members.iter().any(|(_, nested)| *nested) {
        members.iter().rev().fold((quote!(()), quote!(())), |(tail_type, tail), (member, nested)| {
            let (head_type, head) = if *nested {
                (
                    quote!(<#member as #sdk::catalog::Catalog<#visitor>>::Entries),
                    quote!(<#member as #sdk::catalog::Catalog<#visitor>>::ENTRIES),
                )
            } else {
                (quote!([<#visitor as #sdk::catalog::Visitor>::Entry; 1]), quote!([<#visitor as #sdk::catalog::Member<#member>>::ENTRY]))
            };
            (quote!((#head_type, #tail_type)), quote!((#head, #tail)))
        })
    } else {
        let leaves = members.iter().map(|(member, _)| quote!(<#visitor as #sdk::catalog::Member<#member>>::ENTRY));
        (quote!([<#visitor as #sdk::catalog::Visitor>::Entry; #count]), quote!([#(#leaves),*]))
    };
    let (visitor_impl, _, visitor_where) = visitor_generics.split_for_impl();
    let (impl_generics, type_generics, where_clause) = declared.generics.split_for_impl();
    let inspection = [
        quote!(#sdk::catalog::OperationEntry),
        quote!(#sdk::catalog::WireEntry),
        quote!(&'static [#sdk::catalog::WireEntry]),
        quote!(#sdk::catalog::RequirementEntry),
        quote!(#sdk::catalog::ObserverEntry),
        quote!(#sdk::catalog::ListenerEntry),
        quote!(&'static [#sdk::catalog::PublicationEntry]),
    ]
    .into_iter()
    .map(|entry| {
        let mut generics = visitor_generics.clone();
        generics.make_where_clause().predicates.push(syn::parse_quote!(#visitor: #sdk::catalog::Visitor<Entry = #entry>));
        for (member, nested) in &members {
            if *nested {
                generics.make_where_clause().predicates.push(syn::parse_quote!(#member: #sdk::catalog::InertCatalog<#visitor, #entry>));
            }
        }
        let nodes = members.iter().map(|(member, nested)| {
            if *nested {
                quote!(#sdk::catalog::Node::Nested(<#member as #sdk::catalog::InertCatalog<#visitor, #entry>>::ENTRIES))
            } else {
                quote!(#sdk::catalog::Node::Leaf(<#visitor as #sdk::catalog::Member<#member>>::ENTRY))
            }
        });
        let (implementation, _, predicates) = generics.split_for_impl();
        quote! {
            #(#gates)*
            impl #implementation #sdk::catalog::InertCatalog<#visitor, #entry>
                for #name #type_generics #predicates
            {
                const ENTRIES: &'static [#sdk::catalog::Node<#entry>] = &[
                    #(#nodes),*
                ];
            }
        }
    });

    let mut lifetime_name = "__harmos_projection".to_owned();
    while declared.generics.lifetimes().any(|parameter| parameter.lifetime.ident == lifetime_name) {
        lifetime_name.push('_');
    }
    let lifetime = syn::Lifetime::new(&format!("'{lifetime_name}"), proc_macro2::Span::mixed_site());
    let execution_gate = sdk.execution_gate();
    let mut execution_generics = declared.generics.clone();
    execution_generics.make_where_clause().predicates.push(syn::parse_quote!(Self: 'static));
    execution_generics.make_where_clause().predicates.push(syn::parse_quote!(Self: #sdk::catalog::Catalog<Registration>));
    let (execution_impl, _, execution_where) = execution_generics.split_for_impl();
    let execution = quote! {
        #(#gates)*
        #execution_gate
        #[allow(unexpected_cfgs)]
        const _: () = {
            mod execution {
            use super::*;
            pub struct Registration;
            impl #sdk::catalog::Visitor for Registration {
                type Entry = fn(#sdk::__private::RegistrationBatch) -> #sdk::__private::RegistrationBatch;
            }
            impl<O> #sdk::catalog::Member<O> for Registration
            where
                O: #sdk::operations::Executable,
            {
                const ENTRY: Self::Entry = #sdk::__private::RegistrationBatch::operation::<O>;
            }
            impl #execution_impl #name #type_generics #execution_where {
                pub const CATALOG: #sdk::operations::Catalog = #sdk::operations::Catalog::of::<Self>(
                    ::core::stringify!(#name),
                    <Self as #sdk::catalog::InertCatalog<#sdk::operations::Contracts, #sdk::operations::WireDeclaration>>::ENTRIES,
                    <Self as #sdk::catalog::InertCatalog<#sdk::operations::Callees, &'static [#sdk::operations::WireDeclaration]>>::ENTRIES,
                );
            }

            impl #execution_impl #sdk::operations::OperationCatalog for #name #type_generics #execution_where {
                const CATALOG: #sdk::operations::Catalog = Self::CATALOG;

                fn batch() -> #sdk::__private::RegistrationBatch {
                    let mut batch = ::core::option::Option::Some(#sdk::__private::RegistrationBatch::new());
                    let result: ::core::result::Result<(), ::core::convert::Infallible> =
                        #sdk::catalog::Projection::try_for_each(
                            &<Self as #sdk::catalog::Catalog<Registration>>::ENTRIES,
                            |register| {
                                let entries = batch.take().expect("registration projection retains its batch");
                                batch = ::core::option::Option::Some(register(entries));
                                ::core::result::Result::Ok(())
                            },
                        );
                    match result { ::core::result::Result::Ok(()) => {}, ::core::result::Result::Err(error) => match error {} }
                    batch.expect("registration projection retains its batch")
                }
            }
            }
        };
    };
    let execution = (!domain).then_some(execution);
    let methods = quote! {
        #(#gates)*
        const _: () = {
            pub struct Definitions<'a>(::core::marker::PhantomData<&'a ()>);
            impl #sdk::catalog::Visitor for Definitions<'_> {
                type Entry = #sdk::catalog::OperationEntry;
            }
            impl<O: #sdk::OperationDeclaration> #sdk::catalog::Member<O> for Definitions<'_> {
                const ENTRY: Self::Entry = #sdk::catalog::OperationEntry::of::<O>();
            }
            impl #impl_generics #name #type_generics #where_clause {
                pub fn catalog<#lifetime>() -> ::std::vec::Vec<#sdk::OperationDefinition>
                where Self: #sdk::catalog::Catalog<Definitions<#lifetime>>,
                {
                    #sdk::operations::Declarations::from_projection(
                        &<Self as #sdk::catalog::Catalog<Definitions<#lifetime>>>::ENTRIES,
                    ).expect("invalid operation catalog")
                }
            }
        };

        #execution
    };

    let methods = operations.then_some(methods);
    let contains = direct.iter().map(|(member, member_gates)| {
        quote! {
            #(#gates)*
            #(#member_gates)*
            impl #impl_generics #sdk::catalog::Contains<#member> for #name #type_generics #where_clause {}
        }
    });

    let declared_type = emit_declaration.then(|| {
        quote! {
            // Catalog Projection Never Constructs A Member
            #[allow(dead_code, clippy::large_enum_variant, clippy::enum_variant_names)]
            #declared
        }
    });

    Ok(quote! {

        #(#helper_types)*
        #declared_type

        const _: () = {
        #(#helpers)*
        #(#gates)*
        impl #visitor_impl #sdk::catalog::Catalog<#visitor> for #name #type_generics #visitor_where {
            type Entries = #entries_type;
            const ENTRIES: Self::Entries = #entries;
        }

        #(#contains)*

        #(#inspection)*
        #methods
        };
    })
}

fn conditions(attributes: &[syn::Attribute]) -> Result<TokenStream, Error> {
    fn condition(meta: &Meta) -> Result<TokenStream, Error> {
        let Meta::List(list) = meta else {
            return Err(Error::new_spanned(meta, "expected cfg condition"));
        };
        if list.path.is_ident("cfg") {
            return Ok(list.tokens.clone());
        }
        let parts = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        let mut parts = parts.iter();
        let predicate = parts.next().ok_or_else(|| Error::new_spanned(meta, "missing cfg_attr predicate"))?;
        let conditions = parts.map(condition).collect::<Result<Vec<_>, _>>()?;
        Ok(quote!(any(not(#predicate), all(#(#conditions),*))))
    }
    let predicates = attributes.iter().map(|attribute| condition(&attribute.meta)).collect::<Result<Vec<_>, _>>()?;
    match predicates.as_slice() {
        [predicate] => Ok(predicate.clone()),
        predicates => Ok(quote!(all(#(#predicates),*))),
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::declare_domain;
    use quote::quote;

    #[test]
    fn portable_domain_catalog() {
        let expanded = declare_domain(
            proc_macro2::TokenStream::new(),
            quote! {
                enum Shared {
                    Read(crate::Read),
                }
            },
        )
        .to_string();

        assert!(expanded.contains("InertCatalog"));
        assert!(!expanded.contains("RegistrationBatch"));
        assert!(!expanded.contains("OperationCatalog"));
        assert!(!expanded.contains("::operations::Catalog"));
    }
}
