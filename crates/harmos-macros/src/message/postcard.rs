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
use quote::{format_ident, quote};
use syn::{Fields, Item};

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
pub(super) fn declare(item: &Item, sdk: &HarmosPath) -> TokenStream {
    if let Item::Struct(item) = item {
        let name = &item.ident;
        let fields = item.fields.iter().map(|field| {
            let name = field.ident.as_ref().expect("validated named message field");
            let ty = &field.ty;
            quote!(#name: #ty)
        });
        return quote!(#sdk::__private::postcard_product!(#name { #(#fields),* }););
    }
    let mut writes = Vec::new();
    let mut sizes = Vec::new();
    let mut memories = Vec::new();
    let mut reads = Vec::new();
    let Item::Enum(item) = item else {
        unreachable!("validated message shape")
    };
    let name = &item.ident;
    for (index, variant) in item.variants.iter().enumerate() {
        let name = &variant.ident;
        let variant_name = quote!(Self::#name);
        let fields = &variant.fields;
        let index = index as u32;
        let mut members = Vec::new();
        let mut bindings = Vec::new();
        let mut encoded = Vec::new();
        let mut lengths = Vec::new();
        let mut memory = Vec::new();
        let mut decoded = Vec::new();
        for (position, field) in fields.iter().enumerate() {
            let binding = format_ident!("__harmos_value_{position}");
            let member = field.ident.clone().unwrap_or_else(|| binding.clone());
            let ty = &field.ty;
            encoded.push(quote!(<#ty as #sdk::__private::PostcardValue>::write(#binding, bytes);));
            lengths.push(quote!(<#ty as #sdk::__private::PostcardValue>::size(#binding)));
            memory.push(quote! {
                let owned = <#ty as #sdk::__private::PostcardValue>::memory(input)?;
                let bytes = #sdk::__private::decoding_add(bytes, owned.saturating_sub(::std::mem::size_of::<#ty>()))?;
            });
            decoded.push(quote!(<#ty as #sdk::__private::PostcardValue>::read(input)?));
            members.push(member);
            bindings.push(binding);
        }
        let (pattern, value) = match fields {
            Fields::Unit => (quote!(#variant_name), quote!(#variant_name)),
            Fields::Named(_) => (
                quote!(#variant_name { #(#members: #bindings),* }),
                quote!(#variant_name { #(#members: #decoded),* }),
            ),
            Fields::Unnamed(_) => (
                quote!(#variant_name(#(#bindings),*)),
                quote!(#variant_name(#(#decoded),*)),
            ),
        };
        let prefix = quote!(#sdk::__private::PostcardValue::write(&#index, bytes););
        let initial_size = quote!(#sdk::__private::PostcardValue::size(&#index));
        writes.push(quote!(#pattern => { #prefix #(#encoded)* }));
        sizes.push(quote!(#pattern => { let bytes = #initial_size; #(let bytes = bytes.saturating_add(#lengths);)* bytes }));
        memories.push(
            quote!(#index => { let bytes = ::std::mem::size_of::<Self>(); #(#memory)* Ok(bytes) }),
        );
        reads.push(quote!(#index => Ok(#value)));
    }
    let select = quote!(let variant: u32 = input.value()?;);
    quote! {
        impl #sdk::__private::PostcardValue for #name {
            fn write(&self, bytes: &mut ::std::vec::Vec<u8>) { match self { #(#writes),* } }
            fn size(&self) -> usize { match self { #(#sizes),* } }
            fn memory(input: &mut #sdk::__private::PostcardReader<'_>) -> ::std::result::Result<usize, #sdk::MessageError> {
                input.nested(|input| { #select match variant { #(#memories,)* _ => Err(#sdk::MessageError::borrowed("unknown Postcard enum variant")) } })
            }
            fn read(input: &mut #sdk::__private::PostcardReader<'_>) -> ::std::result::Result<Self, #sdk::MessageError> {
                input.nested(|input| { #select match variant { #(#reads,)* _ => Err(#sdk::MessageError::borrowed("unknown Postcard enum variant")) } })
            }
        }
    }
}

/* === Private Functions === */

/* === Tests === */
