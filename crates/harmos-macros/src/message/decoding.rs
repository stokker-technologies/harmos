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
use syn::{GenericArgument, ItemStruct, PathArguments, Type};
// Local Imports
use super::{boxed, tag};
use crate::path::HarmosPath;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) fn declare(item: &ItemStruct, harmos: &HarmosPath) -> TokenStream {
    let mut counts = Vec::new();
    let mut inspections = Vec::new();
    let mut memory = Vec::new();
    let mut nested_memory = Vec::new();
    let mut containers = Vec::new();
    let mut initial = Vec::new();
    let mut merges = Vec::new();
    let mut finish = Vec::new();
    let mut members = Vec::new();
    for member in &item.fields {
        let name = member.ident.as_ref().expect("named message field");
        let authored = name.to_string();
        let field_name = authored.strip_prefix("r#").unwrap_or(&authored);
        let number = tag(field_name);
        let count = format_ident!("__harmos_count_{field_name}");
        let local = format_ident!("__harmos_member_{field_name}");
        let ty = &member.ty;
        let boxed = boxed(ty);
        let (inner, optional, repeated, bytes) = shape(boxed.unwrap_or(ty));
        let scalar = scalar(inner, harmos);
        let string = named(inner, "String");
        let nested = scalar.is_none() && !string && !bytes;
        counts.push(quote!(let mut #count = 0usize;));
        members.push(quote!(#name: #local));

        let inspect = if let Some((kind, _)) = &scalar {
            if repeated {
                quote!(__harmos_field.scalars(#kind, &mut |_| { #count = #harmos::__private::decoding_add(#count, 1)?; ::std::result::Result::Ok(()) })?;)
            } else {
                quote!(__harmos_field.scalar(#kind)?; #count = #harmos::__private::decoding_add(#count, 1)?;)
            }
        } else {
            let validate = if string {
                quote!(__harmos_field.text()?;)
            } else {
                quote!(__harmos_field.bytes()?;)
            };
            quote!(#validate #count = #harmos::__private::decoding_add(#count, 1)?;)
        };
        inspections.push(quote!(#number => { #inspect }));

        if repeated {
            containers.push(quote! {
                let __harmos_layout = ::std::alloc::Layout::array::<#inner>(#count)
                    .map_err(|_| #harmos::__private::MessageError::borrowed("decoding container exceeds address space"))?;
                __harmos_owned = #harmos::__private::decoding_add(__harmos_owned, __harmos_layout.size())?;
            });
            initial
                .push(quote!(let mut #local: #ty = #harmos::__private::decoding_vector(#count)?;));
        } else if !nested {
            initial.push(quote!(let mut #local: #ty = ::std::default::Default::default();));
        }

        if string || bytes {
            let length = if string {
                quote!(__harmos_field.text()?.len())
            } else {
                quote!(__harmos_field.bytes()?.len())
            };
            // Charge Every Occurrence: Replacements May Overlap Until Assignment
            memory.push(quote!(#number => { __harmos_owned = #harmos::__private::decoding_add(__harmos_owned, #length)?; }));
            let value = if string {
                quote!(::std::boxed::Box::<str>::from(__harmos_field.text()?).into_string())
            } else {
                quote!(::std::boxed::Box::<[u8]>::from(__harmos_field.bytes()?).into_vec())
            };
            let merge = if repeated {
                quote!(#local.push(#value);)
            } else if optional {
                quote!(#local = ::std::option::Option::Some(#value);)
            } else {
                quote!(#local = #value;)
            };
            merges.push(quote!(#number => { #merge }));
        } else if let Some((kind, conversion)) = scalar {
            let merge = if repeated {
                quote!(__harmos_field.scalars(#kind, &mut |__harmos_scalar| { #local.push(#conversion); ::std::result::Result::Ok(()) })?;)
            } else if optional {
                quote!(let __harmos_scalar = __harmos_field.scalar(#kind)?; #local = ::std::option::Option::Some(#conversion);)
            } else {
                quote!(let __harmos_scalar = __harmos_field.scalar(#kind)?; #local = #conversion;)
            };
            merges.push(quote!(#number => { #merge }));
        } else if repeated {
            memory.push(quote!(#number => {
                let child = __harmos_input.child(__harmos_field.bytes()?)?;
                let bytes = <#inner as #harmos::__private::Protobuf>::decoding_memory_input(child)?;
                __harmos_owned = #harmos::__private::decoding_add(__harmos_owned, bytes)?;
            }));
            merges.push(quote!(#number => {
                let child = __harmos_input.child(__harmos_field.bytes()?)?;
                let __harmos_scalar = <#inner as #harmos::__private::Protobuf>::decode_input(child)?;
                #local.push(__harmos_scalar);
            }));
        } else {
            // The Nested Bound Includes The Message Resident Size: A Box Pointee Exactly
            let bound = quote! {
                let child = __harmos_input.field(#number)?;
                let bytes = <#inner as #harmos::__private::Protobuf>::decoding_memory_input(child)?;
                __harmos_owned = #harmos::__private::decoding_add(__harmos_owned, bytes)?;
            };
            nested_memory.push(if optional {
                quote!(if #count != 0 { #bound })
            } else {
                bound
            });
            let value = quote!(<#inner as #harmos::__private::Protobuf>::decode_input(__harmos_input.field(#number)?)?);
            // Allocate The Box Only After Its Message Decoded Successfully
            let value = if boxed.is_some() {
                quote!(::std::boxed::Box::new(#value))
            } else {
                value
            };
            finish.push(if optional {
                quote!(let #local: #ty = if #count == 0 { ::std::option::Option::None } else { ::std::option::Option::Some(#value) };)
            } else {
                quote!(let #local: #ty = #value;)
            });
        }
    }
    let resident = if containers.is_empty() && memory.is_empty() && nested_memory.is_empty() {
        quote!(let __harmos_owned = ::std::mem::size_of::<Self>();)
    } else {
        quote!(let mut __harmos_owned = ::std::mem::size_of::<Self>();)
    };
    let inspect = quote! {
        #(#counts)*
        __harmos_input.fields(&mut |__harmos_field| {
            match __harmos_field.tag { #(#inspections,)* _ => {} }
            ::std::result::Result::Ok(())
        })?;
    };
    quote! {
        fn decoding_memory(bytes: &[::std::primitive::u8]) -> ::std::result::Result<usize, #harmos::__private::MessageError> {
            <Self as #harmos::__private::Protobuf>::decoding_memory_input(#harmos::__private::DecodeInput::new(bytes))
        }
        fn decoding_memory_input(__harmos_input: #harmos::__private::DecodeInput<'_>) -> ::std::result::Result<usize, #harmos::__private::MessageError> {
            #inspect
            #resident
            #(#containers)*
            __harmos_input.fields(&mut |__harmos_field| {
                match __harmos_field.tag { #(#memory,)* _ => {} }
                ::std::result::Result::Ok(())
            })?;
            #(#nested_memory)*
            ::std::result::Result::Ok(__harmos_owned)
        }
        fn decode(bytes: &[::std::primitive::u8]) -> ::std::result::Result<Self, #harmos::__private::MessageError> {
            <Self as #harmos::__private::Protobuf>::decode_input(#harmos::__private::DecodeInput::new(bytes))
        }
        fn decode_input(__harmos_input: #harmos::__private::DecodeInput<'_>) -> ::std::result::Result<Self, #harmos::__private::MessageError> {
            #inspect
            #(#initial)*
            __harmos_input.fields(&mut |__harmos_field| {
                match __harmos_field.tag { #(#merges,)* _ => {} }
                ::std::result::Result::Ok(())
            })?;
            #(#finish)*
            ::std::result::Result::Ok(Self { #(#members),* })
        }
    }
}

/* === Private Functions === */
fn named(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|part| part.ident == name))
}

fn shape(ty: &Type) -> (&Type, bool, bool, bool) {
    let Type::Path(path) = ty else {
        unreachable!("validated message shape");
    };
    let part = path.path.segments.last().expect("message type");
    let optional = part.ident == "Option";
    let repeated = part.ident == "Vec";
    if !optional && !repeated {
        return (ty, false, false, false);
    }
    let PathArguments::AngleBracketed(arguments) = &part.arguments else {
        unreachable!();
    };
    let Some(GenericArgument::Type(inner)) = arguments.args.first() else {
        unreachable!();
    };
    let bytes = repeated && named(inner, "u8");
    (inner, optional, repeated && !bytes, bytes)
}

fn scalar(ty: &Type, harmos: &HarmosPath) -> Option<(TokenStream, TokenStream)> {
    let Type::Path(path) = ty else {
        return None;
    };
    let part = path.path.segments.last()?;
    let (kind, convert) = match part.ident.to_string().as_str() {
        "bool" => (quote!(Varint), quote!(__harmos_scalar != 0)),
        "u32" => (quote!(Varint), quote!(__harmos_scalar as u32)),
        "u64" => (quote!(Varint), quote!(__harmos_scalar)),
        "i32" => (quote!(Varint), quote!(__harmos_scalar as i32)),
        "i64" => (quote!(Varint), quote!(__harmos_scalar as i64)),
        "f32" => (quote!(Bits32), quote!(f32::from_bits(__harmos_scalar))),
        "f64" => (quote!(Bits64), quote!(f64::from_bits(__harmos_scalar))),
        _ => return None,
    };
    let conversion = quote!(match __harmos_scalar {
        #harmos::__private::DecodeValue::#kind(__harmos_scalar) => #convert,
        _ => unreachable!("validated scalar wire type"),
    });
    Some((quote!(#harmos::__private::DecodeScalar::#kind), conversion))
}

/* === Tests === */
