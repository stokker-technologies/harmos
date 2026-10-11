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
use syn::{Error, Fields, ItemEnum, Type, spanned::Spanned};
// Local Imports
use super::{encoding, tag};
use crate::path::HarmosPath;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) fn expand(
    mut item: ItemEnum,
    harmos: HarmosPath,
    skip_debug: bool,
) -> Result<TokenStream, Error> {
    if !item.generics.params.is_empty() || item.variants.is_empty() {
        return Err(Error::new(
            item.span(),
            "a message enum requires non-generic variants",
        ));
    }
    if item.attrs.iter().any(|attr| attr.path().is_ident("prost")) {
        return Err(Error::new(
            item.span(),
            "message owns protobuf attributes and tags",
        ));
    }
    let postcard = super::postcard::declare(&syn::Item::Enum(item.clone()), &harmos);
    let name = &item.ident;
    let mut tags = BTreeMap::new();
    let mut payloads = Vec::new();
    let mut encoders = Vec::new();
    let mut lengths = Vec::new();
    let mut decoders = Vec::new();
    let mut memories = Vec::new();
    let mut schemas = Vec::new();
    let mut defaults = Vec::new();
    let mut bounds = Vec::new();
    let mut wire_tags = Vec::new();
    let mut defaultable = true;
    for (index, variant) in item.variants.iter_mut().enumerate() {
        if variant.discriminant.is_some()
            || variant
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("prost"))
        {
            return Err(Error::new(
                variant.span(),
                "message variants use name-derived tags, without discriminants or prost attributes",
            ));
        }
        let variant_name = &variant.ident;
        let authored = variant_name.to_string();
        let authored = authored.strip_prefix("r#").unwrap_or(&authored);
        let number = tag(authored);
        if let Some(previous) = tags.insert(number, authored.to_owned()) {
            return Err(Error::new(
                variant.span(),
                format!(
                    "message variants `{previous}` and `{authored}` collide at protobuf tag {number}; rename a variant explicitly"
                ),
            ));
        }
        let payload = format_ident!("Payload{index}");
        let mut declarations = Vec::new();
        let mut bindings = Vec::new();
        let mut members = Vec::new();
        let mut writes = BTreeMap::new();
        let mut sizes = Vec::new();
        let mut owned = Vec::new();
        let mut read = Vec::new();
        let mut initial = Vec::new();
        for (position, field) in variant.fields.iter_mut().enumerate() {
            let ident = field
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("field_{position}"));
            let ty = &field.ty;
            // Validate The Same Portable Field Shapes As Struct Messages
            let (wire, _, _) = encoding(ty, &harmos)?;
            if index == 0
                && wire.to_string().starts_with("message")
                && wire.to_string().contains("required")
            {
                defaultable = false;
            }
            let field_name = ident.to_string();
            let field_name = field_name.strip_prefix("r#").unwrap_or(&field_name);
            let field_tag = tag(field_name);
            let binding = format_ident!("__harmos_field_{position}");
            let (write, size) = codec(ty, &binding, field_tag, &harmos)?;
            let wire = wire.to_string();
            let defaulted = !wire.starts_with("message")
                && !wire.contains("optional")
                && !wire.contains("repeated")
                && !wire.contains("packed");
            let write = if defaulted {
                quote!(if *#binding != <#ty as ::std::default::Default>::default() { #write })
            } else {
                write
            };
            let size = if defaulted {
                quote!(if *#binding == <#ty as ::std::default::Default>::default() { 0 } else { #size })
            } else {
                size
            };
            writes.insert(field_tag, write);
            sizes.push(size);
            owned.push(quote! {
                let owned = #harmos::MemorySize::memory_size(#binding);
                if owned == usize::MAX { return usize::MAX; }
                let bytes = bytes.saturating_add(owned.saturating_sub(::std::mem::size_of_val(#binding)));
            });
            let attrs = std::mem::take(&mut field.attrs);
            declarations.push(quote!(#(#attrs)* #ident: #ty));
            bindings.push(binding);
            members.push(ident.clone());
            read.push(quote!(payload.#ident));
            initial.push(quote!(::std::default::Default::default()));
        }
        let pattern = match &variant.fields {
            Fields::Unit => quote!(#name::#variant_name),
            Fields::Unnamed(_) => quote!(#name::#variant_name(#(#bindings),*)),
            Fields::Named(_) => quote!(#name::#variant_name { #(#members: #bindings),* }),
        };
        let decoded = match &variant.fields {
            Fields::Unit => quote!(#name::#variant_name),
            Fields::Unnamed(_) => quote!(#name::#variant_name(#(#read),*)),
            Fields::Named(_) => quote!(#name::#variant_name { #(#members: #read),* }),
        };
        let default = match &variant.fields {
            Fields::Unit => quote!(#name::#variant_name),
            Fields::Unnamed(_) => quote!(#name::#variant_name(#(#initial),*)),
            Fields::Named(_) => quote!(#name::#variant_name { #(#members: #initial),* }),
        };
        defaults.push(default);
        let declaration = super::expand(
            quote!(harmos_path = #harmos),
            quote!(struct #payload { #(#declarations),* }),
        )?;
        payloads.push(declaration);
        let writes = writes.into_values();
        encoders.push(quote! { #pattern => {
            let length = 0usize #(+ #sizes)*;
            #harmos::__private::prost::encoding::encode_key(#number, #harmos::__private::prost::encoding::WireType::LengthDelimited, buf);
            #harmos::__private::prost::encoding::encode_varint(length as u64, buf);
            #(#writes)*
        }});
        lengths.push(quote! { #pattern => {
            let length = 0usize #(+ #sizes)*;
            #harmos::__private::prost::encoding::key_len(#number) + #harmos::__private::prost::encoding::encoded_len_varint(length as u64) + length
        }});
        wire_tags.push(number);
        decoders.push(quote! { #number => {
            let payload = <#payload as #harmos::__private::Protobuf>::decode_input(selected)?;
            #decoded
        }});
        bounds.push(
            quote! { #number => <#payload as #harmos::__private::Protobuf>::decoding_memory_input(selected)? },
        );
        memories.push(
            quote! { #pattern => { let bytes = ::std::mem::size_of::<Self>(); #(#owned)* bytes } },
        );
        schemas.push(quote! { #harmos::schema::Field {
            name: ::std::borrow::Cow::Borrowed(#authored), tag: #number,
            kind: #harmos::schema::Kind::Message { fields: ::std::borrow::Cow::Borrowed(<#payload as #harmos::schema::Described>::FIELDS) },
            cardinality: #harmos::schema::Cardinality::Variant,
        }});
    }
    let derives: Vec<_> = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .map(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    let has_default = derives.iter().any(|path| {
        path.segments
            .last()
            .is_some_and(|part| part.ident == "Default")
    });
    let unit_defaults: Vec<_> = item
        .variants
        .iter()
        .filter(|variant| {
            matches!(variant.fields, Fields::Unit)
                && variant
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("default"))
        })
        .collect();
    let unit_default = if has_default && unit_defaults.len() == 1 {
        Some(&unit_defaults[0].ident)
    } else {
        None
    };
    if !skip_debug
        && !derives.iter().any(|path| {
            path.segments
                .last()
                .is_some_and(|part| part.ident == "Debug")
        })
    {
        item.attrs.push(syn::parse_quote!(#[derive(Debug)]));
    }
    let default = &defaults[0];
    let default_impl = (!has_default && defaultable).then(|| {
        quote! {
            impl ::std::default::Default for #name { fn default() -> Self { #default } }
        }
    });
    let first_variant = &item.variants[0];
    let first = &first_variant.ident;
    let first_payload = format_ident!("Payload0");
    let first_fields = first_variant
        .fields
        .iter()
        .enumerate()
        .map(|(position, field)| {
            field
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("field_{position}"))
        })
        .collect::<Vec<_>>();
    let empty = match first_variant.fields {
        Fields::Unit => quote!(#name::#first),
        Fields::Unnamed(_) => quote!(#name::#first(#(payload.#first_fields),*)),
        Fields::Named(_) => quote!(#name::#first { #(#first_fields: payload.#first_fields),* }),
    };
    let empty = if let Some(variant) = unit_default {
        quote!(#name::#variant)
    } else if has_default {
        quote!(::std::default::Default::default())
    } else {
        quote!({
            let payload = <#first_payload as #harmos::__private::Protobuf>::decode_input(input.child(&[])?)?;
            let _ = &payload;
            #empty
        })
    };
    let empty_bound = if unit_default.is_some() {
        quote!(0usize)
    } else if has_default {
        quote!(return Err(#harmos::__private::MessageError::borrowed("custom enum default decoding memory bound is unsupported")))
    } else {
        quote!(<#first_payload as #harmos::__private::Protobuf>::decoding_memory_input(input.child(&[])?)?)
    };
    let schemas = schemas.into_iter();
    Ok(quote! {
        #item
        #postcard
        const _: () = {
            #(#payloads)*
            #default_impl
            #harmos::__private::with_protobuf! {
            impl #harmos::__private::Protobuf for #name {
                fn encode_to_vec(&self) -> ::std::vec::Vec<u8> {
                    let mut bytes = ::std::vec::Vec::with_capacity(#harmos::__private::Protobuf::encoded_len(self));
                    #harmos::__private::Protobuf::encode_into(self, &mut bytes);
                    bytes
                }
                fn encode_into(&self, buf: &mut ::std::vec::Vec<u8>) {
                    match self { #(#encoders),* }
                }
                fn encoded_len(&self) -> usize { match self { #(#lengths),* } }
                fn decoding_memory(bytes: &[u8]) -> ::std::result::Result<usize, #harmos::__private::MessageError> {
                    <Self as #harmos::__private::Protobuf>::decoding_memory_input(#harmos::__private::DecodeInput::new(bytes))
                }
                fn decoding_memory_input(input: #harmos::__private::DecodeInput<'_>) -> ::std::result::Result<usize, #harmos::__private::MessageError> {
                    let bytes = match input.variant(&[#(#wire_tags),*])? {
                        Some((tag, selected)) => match tag { #(#bounds,)* _ => unreachable!("selected declared variant") },
                        None => { #empty_bound },
                    };
                    #harmos::__private::decoding_add(::std::mem::size_of::<Self>(), bytes)
                }
                fn decode(bytes: &[u8]) -> ::std::result::Result<Self, #harmos::__private::MessageError> {
                    <Self as #harmos::__private::Protobuf>::decode_input(#harmos::__private::DecodeInput::new(bytes))
                }
                fn decode_input(input: #harmos::__private::DecodeInput<'_>) -> ::std::result::Result<Self, #harmos::__private::MessageError> {
                    ::std::result::Result::Ok(match input.variant(&[#(#wire_tags),*])? {
                        Some((tag, selected)) => match tag { #(#decoders,)* _ => unreachable!("selected declared variant") },
                        None => #empty,
                    })
                }
            }
            }
            impl #harmos::MemorySize for #name {
                fn memory_size(&self) -> usize {
                    match self { #(#memories),* }
                }
            }
            impl #harmos::RowMetadata for #name { const ROW_METADATA_JSON: &'static str = "[]"; }
            impl #harmos::schema::Described for #name {
                const FIELDS: &'static [#harmos::schema::Field] = &[#(#schemas),*];
            }
        };
    })
}

pub(super) fn codec(
    ty: &Type,
    value: &syn::Ident,
    tag: u32,
    harmos: &HarmosPath,
) -> Result<(TokenStream, TokenStream), Error> {
    let (wire, _, _) = encoding(ty, harmos)?;
    let attributes = syn::parse::Parser::parse2(
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        wire,
    )?;
    let codec = attributes
        .first()
        .and_then(|meta| meta.path().get_ident())
        .expect("encoding starts with a codec");
    let path = quote!(#harmos::__private::prost::encoding::#codec);
    let optional = attributes
        .iter()
        .any(|meta| meta.path().is_ident("optional"));
    let packed = attributes.iter().any(|meta| meta.path().is_ident("packed"));
    let repeated = attributes
        .iter()
        .any(|meta| meta.path().is_ident("repeated"));
    if codec == "message" {
        let write = quote! {
            let length = #harmos::__private::Protobuf::encoded_len(value);
            #harmos::__private::prost::encoding::encode_key(#tag, #harmos::__private::prost::encoding::WireType::LengthDelimited, buf);
            #harmos::__private::prost::encoding::encode_varint(length as u64, buf);
            #harmos::__private::Protobuf::encode_into(value, buf);
        };
        let size = quote!({
            let length = #harmos::__private::Protobuf::encoded_len(value);
            #harmos::__private::prost::encoding::key_len(#tag)
                .saturating_add(#harmos::__private::prost::encoding::encoded_len_varint(length as u64))
                .saturating_add(length)
        });
        return Ok(if optional {
            (
                quote!(if let ::std::option::Option::Some(value) = #value { #write }),
                quote!(#value.as_ref().map_or(0, |value| #size)),
            )
        } else if repeated {
            (
                quote!(for value in #value { #write }),
                quote!(#value.iter().fold(0usize, |length, value| length.saturating_add(#size))),
            )
        } else {
            (
                quote!({ let value = #value; #write }),
                quote!({ let value = #value; #size }),
            )
        });
    }
    if optional {
        return Ok((
            quote!(if let ::std::option::Option::Some(value) = #value { #path::encode(#tag, value, buf); }),
            quote!(#value.as_ref().map_or(0, |value| #path::encoded_len(#tag, value))),
        ));
    }
    let (encode, length) = if packed {
        (
            format_ident!("encode_packed"),
            format_ident!("encoded_len_packed"),
        )
    } else if repeated {
        (
            format_ident!("encode_repeated"),
            format_ident!("encoded_len_repeated"),
        )
    } else {
        (format_ident!("encode"), format_ident!("encoded_len"))
    };
    Ok((
        quote!(#path::#encode(#tag, #value, buf);),
        quote!(#path::#length(#tag, #value)),
    ))
}

/* === Private Functions === */

/* === Tests === */
