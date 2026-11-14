/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use syn::{Attribute, Error, Meta, Token, punctuated::Punctuated};

pub(crate) fn configuration(attributes: &[Attribute]) -> Result<Vec<Attribute>, Error> {
    fn select(meta: &Meta) -> Result<Option<Meta>, Error> {
        if meta.path().is_ident("cfg") {
            return Ok(Some(meta.clone()));
        }
        let Meta::List(list) = meta else {
            return Ok(None);
        };
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
