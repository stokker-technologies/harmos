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
use syn::{Attribute, Error, Meta, Token, Type};
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
    expand(attribute, item).unwrap_or_else(Error::into_compile_error)
}

/* === Private Functions === */

fn arguments(input: syn::parse::ParseStream<'_>) -> Result<Punctuated<Meta, Token![,]>, Error> {
    input.parse_terminated(argument, Token![,])
}

fn argument(input: syn::parse::ParseStream<'_>) -> Result<Meta, Error> {
    let fork = input.fork();
    let path: syn::Path = fork.parse()?;
    if ["request", "response", "input", "output"].iter().any(|name| path.is_ident(name)) {
        let path = input.parse()?;
        let eq_token = input.parse()?;
        let value: Type = input.parse()?;
        return Ok(Meta::NameValue(syn::MetaNameValue { path, eq_token, value: syn::Expr::Verbatim(quote!(#value)) }));
    }
    input.parse()
}

fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let declaration: syn::ItemStruct = syn::parse2(item).map_err(|error| {
        Error::new(
            error.span(),
            "operation belongs on the implementation struct; implement Operation explicitly under cfg(feature = \"runtime\")",
        )
    })?;
    let mut explicit = None;
    let mut metadata = Vec::new();
    for argument in arguments.parse2(attribute)? {
        if let Some(value) = path::declared(&argument) {
            if explicit.replace(value?).is_some() {
                return Err(Error::new_spanned(argument, "harmos_path may be declared only once"));
            }
        } else {
            metadata.push(argument);
        }
    }
    let sdk = HarmosPath::resolve(explicit);
    crate::catalog::operation::structure(quote!(#(#metadata),*), declaration, &sdk)
}

pub(crate) fn configuration(attributes: &[Attribute]) -> Result<Vec<Attribute>, Error> {
    fn select(meta: &Meta) -> Result<Option<Meta>, Error> {
        if meta.path().is_ident("cfg") {
            return Ok(Some(meta.clone()));
        }
        let Meta::List(list) = meta else { return Ok(None) };
        if !list.path.is_ident("cfg_attr") {
            return Ok(None);
        }
        let arguments = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        let mut arguments = arguments.iter();
        let predicate = arguments.next().ok_or_else(|| Error::new_spanned(list, "cfg_attr requires a predicate and attributes"))?;
        let selected = arguments.filter_map(|meta| select(meta).transpose()).collect::<Result<Vec<_>, _>>()?;
        if selected.is_empty() {
            return Ok(None);
        }
        Ok(Some(syn::parse_quote!(cfg_attr(#predicate, #(#selected),*))))
    }
    attributes
        .iter()
        .filter_map(|attribute| select(&attribute.meta).transpose().map(|meta| meta.map(|meta| syn::parse_quote!(#[#meta]))))
        .collect()
}

/* === Tests === */
