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
use syn::{Error, ItemStruct};
// Local Imports
use super::encoding;
use crate::path::HarmosPath;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
// The Authored Struct With Its Message Debug And Default Surface. Transport
// Codecs Derive From Its Postcard Value And Described Shape Elsewhere.
pub(super) fn declare(
    item: &mut ItemStruct,
    harmos: &HarmosPath,
    skip_debug: bool,
) -> Result<TokenStream, Error> {
    let mut defaultable = true;
    for field in &item.fields {
        let (required_message, _, _) = encoding(&field.ty, harmos)?;
        if required_message {
            defaultable = false;
        }
    }
    if !skip_debug && !derives(item, "Debug") {
        item.attrs.push(syn::parse_quote!(#[derive(Debug)]));
    }
    if defaultable && !derives(item, "Default") {
        item.attrs.push(syn::parse_quote!(#[derive(Default)]));
    }
    Ok(quote!(#item))
}

/* === Private Functions === */
fn derives(item: &ItemStruct, name: &str) -> bool {
    item.attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .any(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|paths| {
                paths
                    .iter()
                    .any(|path| path.segments.last().is_some_and(|part| part.ident == name))
            })
        })
}

/* === Tests === */
