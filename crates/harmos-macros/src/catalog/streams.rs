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
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, ItemEnum, Meta, Path, Token, Type};
// Local Imports
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Member {
    definition: Type,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn close(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, item.clone()) {
        Ok(closed) => closed,
        Err(refused) => {
            let refusal = refused.to_compile_error();

            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */

fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let harmos = harmos_path(attribute)?;

    let catalog: ItemEnum = syn::parse2(item)?;
    let members = members(&catalog)?;
    let name = &catalog.ident;
    let visibility = &catalog.vis;
    let memberships = members.iter().map(|member| {
        let definition = &member.definition;

        quote! {
            #[automatically_derived]
            impl #harmos::__private::StreamCatalog<#definition> for #name {}
        }
    });
    let registrations = members.iter().map(|member| {
        let definition = &member.definition;

        quote!(<#definition>::definition())
    });
    let dispatch = members.iter().map(|member| {
        let definition = &member.definition;

        quote! {
            if id == <#definition>::DEFINITION.0 {
                return ::core::option::Option::Some(
                    <#definition>::definition(),
                );
            }
        }
    });

    let catalog = crate::catalog::typed::projection(quote!(harmos_path = #harmos), quote!(#[allow(dead_code)] #catalog))?;
    Ok(quote! {
        #catalog

        #(#memberships)*

        impl #name {

            #visibility fn catalog() -> ::std::vec::Vec<#harmos::streams::Definition> {
                ::std::vec![#(#registrations),*]
            }


            #visibility fn definition(id: &str) -> ::core::option::Option<#harmos::streams::Definition> {
                #(#dispatch)*

                ::core::option::Option::None
            }
        }
    })
}

fn members(catalog: &ItemEnum) -> Result<Vec<Member>, Error> {
    let mut members = Vec::new();

    for variant in &catalog.variants {
        let syn::Fields::Unnamed(fields) = &variant.fields else {
            return Err(Error::new(variant.span(), "a stream catalog variant names exactly one row type"));
        };
        let [field] = &fields.unnamed.iter().collect::<Vec<_>>()[..] else {
            return Err(Error::new(variant.span(), "a stream catalog variant names exactly one row type"));
        };

        members.push(Member { definition: field.ty.clone() });
    }

    if members.is_empty() {
        return Err(Error::new(catalog.span(), "a stream catalog declares at least one row type"));
    }

    Ok(members)
}

fn harmos_path(attribute: TokenStream) -> Result<HarmosPath, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut harmos_path: Option<Path> = None;

    for argument in &arguments {
        match path::declared(argument) {
            Some(parsed) => set_once(&mut harmos_path, parsed?, argument.span(), path::KEY)?,
            None => {
                return Err(Error::new(
                    argument.span(),
                    "a stream catalog declares no arguments of its own; the application names it \
                     through `Application::Streams`, and a stream catalog re-exported through a \
                     facade crate declares `harmos_path = …`",
                ));
            }
        }
    }

    Ok(HarmosPath::resolve(harmos_path))
}

fn set_once<T>(place: &mut Option<T>, value: T, span: proc_macro2::Span, name: &str) -> Result<(), Error> {
    if place.replace(value).is_some() {
        return Err(Error::new(span, format!("`{name}` is declared only once")));
    }

    Ok(())
}

/* === Tests === */
