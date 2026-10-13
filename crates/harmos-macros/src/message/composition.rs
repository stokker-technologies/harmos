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
use std::collections::BTreeMap;
// Third Party Imports
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, ItemStruct, Type};
// Local Imports
use super::{boxed, encoding, enumeration, tag};
use crate::path::HarmosPath;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) fn declare(
    item: &mut ItemStruct,
    harmos: &HarmosPath,
    skip_debug: bool,
) -> Result<TokenStream, Error> {
    let name = &item.ident;
    let mut writes = BTreeMap::new();
    let mut sizes = Vec::new();
    let mut defaultable = true;
    for field in &mut item.fields {
        let ident = field.ident.as_ref().expect("named message field");
        let text = ident.to_string();
        let authored = text.strip_prefix("r#").unwrap_or(&text);
        let number = tag(authored);
        let ty = &field.ty;
        let (wire, _, _) = encoding(ty, harmos)?;
        field.attrs.retain(|attr| !attr.path().is_ident("prost"));
        let value = format_ident!("value");
        if !wire.to_string().starts_with("message") {
            let (write, size) = enumeration::codec(ty, &value, number, harmos)?;
            let defaulted = !wire.to_string().contains("optional")
                && !wire.to_string().contains("repeated")
                && !wire.to_string().contains("packed");
            let write = if defaulted {
                quote!(if *value != <#ty as ::std::default::Default>::default() { #write })
            } else {
                write
            };
            let size = if defaulted {
                quote!(if *value == <#ty as ::std::default::Default>::default() { 0 } else { #size })
            } else {
                size
            };
            writes.insert(number, quote!({ let value = &self.#ident; #write }));
            sizes.push(quote!({ let value = &self.#ident; #size }));
            continue;
        }
        let Type::Path(path) = ty else {
            unreachable!("validated message path")
        };
        let segment = path.path.segments.last().expect("message type");
        let optional = segment.ident == "Option";
        let repeated = segment.ident == "Vec";
        if !optional && !repeated {
            defaultable = false;
        }
        let write = quote! {
            let length = #harmos::__private::Protobuf::encoded_len(value);
            #harmos::__private::prost::encoding::encode_key(#number, #harmos::__private::prost::encoding::WireType::LengthDelimited, buf);
            #harmos::__private::prost::encoding::encode_varint(length as u64, buf);
            #harmos::__private::Protobuf::encode_into(value, buf);
        };
        let size = quote!({
            let length = #harmos::__private::Protobuf::encoded_len(value);
            #harmos::__private::prost::encoding::key_len(#number)
                .saturating_add(#harmos::__private::prost::encoding::encoded_len_varint(length as u64))
                .saturating_add(length)
        });
        if optional {
            writes.insert(
                number,
                quote!(if let ::std::option::Option::Some(value) = &self.#ident { #write }),
            );
            sizes.push(quote!(self.#ident.as_ref().map_or(0, |value| #size)));
        } else if repeated {
            writes.insert(number, quote!(for value in &self.#ident { #write }));
            sizes.push(quote!(self.#ident.iter().fold(0usize, |length, value| length.saturating_add(#size))));
        } else if let Some(inner) = boxed(ty) {
            // Protobuf Calls Do Not Autoderef: Encode Through The Boxed Message
            writes.insert(
                number,
                quote!({ let value: &#inner = &*self.#ident; #write }),
            );
            sizes.push(quote!({ let value: &#inner = &*self.#ident; #size }));
        } else {
            writes.insert(number, quote!({ let value = &self.#ident; #write }));
            sizes.push(quote!({ let value = &self.#ident; #size }));
        }
    }
    // Match The Debug Surface Previously Supplied By The Protobuf Derive
    let derives_debug = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .any(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|paths| {
                paths.iter().any(|path| {
                    path.segments
                        .last()
                        .is_some_and(|part| part.ident == "Debug")
                })
            })
        });
    if !skip_debug && !derives_debug {
        item.attrs.push(syn::parse_quote!(#[derive(Debug)]));
    }
    if defaultable {
        let has_default = item
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("derive"))
            .any(|attr| {
                attr.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths.iter().any(|path| {
                        path.segments
                            .last()
                            .is_some_and(|part| part.ident == "Default")
                    })
                })
            });
        if !has_default {
            item.attrs.push(syn::parse_quote!(#[derive(Default)]));
        }
    }
    let writes = writes.into_values();
    let decoding = super::decoding::declare(item, harmos);
    Ok(quote! {
        #item
        #harmos::__private::with_protobuf! {
        const _: () = {
            impl #harmos::__private::Protobuf for #name {
                fn encode_to_vec(&self) -> ::std::vec::Vec<u8> {
                    let mut bytes = ::std::vec::Vec::with_capacity(#harmos::__private::Protobuf::encoded_len(self));
                    #harmos::__private::Protobuf::encode_into(self, &mut bytes);
                    bytes
                }
                fn encode_into(&self, buf: &mut ::std::vec::Vec<u8>) {
                    #(#writes)*
                }
                fn encoded_len(&self) -> usize {
                    let length = 0usize;
                    #(let length = length.saturating_add(#sizes);)*
                    length
                }
                #decoding
            }
        };
        }
    })
}

/* === Private Functions === */

/* === Tests === */
