/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Request Shape Derive

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Error, Type};

// Local Imports
use crate::path::HarmosPath;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn derive(input: TokenStream) -> TokenStream {
    expand(input).unwrap_or_else(Error::into_compile_error)
}

/* === Private Functions === */

fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(input)?;
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(&input.generics, "a request cannot be generic"));
    }
    if !matches!(input.data, syn::Data::Struct(_)) {
        return Err(Error::new_spanned(&input, "a request requires a struct"));
    }
    let mut explicit = None;
    let mut response: Option<Type> = None;
    let mut incoming: Option<Type> = None;
    let mut outgoing: Option<Type> = None;
    for attribute in &input.attrs {
        if attribute.path().is_ident("harmos_path") {
            crate::set_once(&mut explicit, attribute.parse_args()?, attribute.pound_token.span, "harmos_path")?;
        }
        if attribute.path().is_ident("request") {
            attribute.parse_nested_meta(|meta| {
                let target = if meta.path.is_ident("response") {
                    &mut response
                } else if meta.path.is_ident("input") {
                    &mut incoming
                } else if meta.path.is_ident("output") {
                    &mut outgoing
                } else {
                    return Err(meta.error("expected response, input or output"));
                };
                if target.replace(meta.value()?.parse()?).is_some() {
                    return Err(meta.error("request shape fields cannot repeat"));
                }
                Ok(())
            })?;
        }
    }
    let harmos = HarmosPath::resolve(explicit);
    let name = &input.ident;
    let response = response.unwrap_or_else(|| syn::parse_quote!(()));
    let direction =
        |ty: Option<Type>| ty.map_or_else(|| quote!(#harmos::operations::NoStream), |ty| quote!(#harmos::operations::Stream<#ty>));
    let incoming = direction(incoming);
    let outgoing = direction(outgoing);
    Ok(quote! {
        impl #harmos::operations::RequestShape for #name {
            type Request = Self;
            type Response = #response;
            type Input = #incoming;
            type Output = #outgoing;
        }
    })
}

/* === Tests === */
