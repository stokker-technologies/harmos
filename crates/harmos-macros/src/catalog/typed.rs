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
    match expand(attribute, item) {
        Ok(expanded) => expanded,
        Err(error) => error.to_compile_error(),
    }
}

/* === Private Functions === */

fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
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
    let sdk = explicit.map(|path| HarmosPath::resolve(Some(path))).unwrap_or_else(HarmosPath::catalog);

    let mut declared: ItemEnum = syn::parse2(item)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut variants = std::collections::BTreeSet::new();
    let mut members = Vec::new();
    for variant in &mut declared.variants {
        if !variants.insert(variant.ident.to_string()) {
            return Err(Error::new_spanned(&variant.ident, "a catalog variant name may appear only once"));
        }
        if variant.discriminant.is_some()
            || variant.attrs.iter().any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        {
            return Err(Error::new_spanned(variant, "catalog variants cannot have discriminants or conditional attributes"));
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
        members.push((member.clone(), nested));
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
    let direct = members.iter().filter_map(|(member, nested)| (!nested).then_some(member));
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
            impl #implementation #sdk::catalog::InertCatalog<#visitor, #entry>
                for #name #type_generics #predicates
            {
                const ENTRIES: &'static [#sdk::catalog::Node<#entry>] = &[
                    #(#nodes),*
                ];
            }
        }
    });

    Ok(quote! {

        // Catalog variants name types; projecting them never constructs a value.
        #[allow(dead_code, clippy::large_enum_variant, clippy::enum_variant_names)]
        #declared

        impl #visitor_impl #sdk::catalog::Catalog<#visitor> for #name #type_generics #visitor_where {
            type Entries = #entries_type;
            const ENTRIES: Self::Entries = #entries;
        }

        #(impl #impl_generics #sdk::catalog::Contains<#direct> for #name #type_generics #where_clause {})*

        #(#inspection)*
    })
}

/* === Tests === */
