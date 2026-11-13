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
use syn::{Error, Item};

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

pub(crate) fn derive(item: TokenStream) -> TokenStream {
    expand(item).unwrap_or_else(Error::into_compile_error)
}

/* === Private Functions === */

fn expand(item: TokenStream) -> Result<TokenStream, Error> {
    let mut parsed: Item = syn::parse2(item.clone())?;
    let attributes = match &parsed {
        Item::Struct(shape) => &shape.attrs,
        Item::Enum(shape) => &shape.attrs,
        other => return Err(Error::new_spanned(other, "Row requires a struct or enum")),
    };
    let explicit = attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("harmos_path"))
        .map(|attribute| {
            attribute
                .parse_args::<syn::Path>()
                .map_err(|error| Error::new_spanned(attribute, format!("`harmos_path` names the crate path: {error}")))
        })
        .transpose()?;
    let harmos = HarmosPath::resolve(explicit);
    if crate::stored_message::portable(&parsed, &harmos) {
        return crate::stored_message::expand(quote!(harmos_path = #harmos), item);
    }
    let schema = crate::stored_message::shape(&parsed, &harmos)?;
    let (name, metadata) = match &mut parsed {
        Item::Struct(shape) => (&shape.ident, crate::presentation::fields(&mut shape.fields)?),
        Item::Enum(shape) => (&shape.ident, "[]".to_owned()),
        _ => unreachable!(),
    };
    Ok(quote! {
        #schema
        impl #harmos::RowMetadata for #name {
            const ROW_METADATA_JSON: &'static str = #metadata;
        }
    })
}

/* === Tests === */
