/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod composition;
mod enumeration;
mod postcard;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
// Third Party Imports
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Error, Fields, GenericArgument, Meta, PathArguments, Token, Type, parse::Parser, punctuated::Punctuated, spanned::Spanned};
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

pub(crate) fn declare(attributes: TokenStream, item: TokenStream) -> TokenStream {
    expand(attributes, item).unwrap_or_else(Error::into_compile_error)
}

pub(crate) fn expand(attributes: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let mut explicit = None;
    let mut skip_debug = None;
    let mut skip_default = None;
    for meta in Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attributes)? {
        if matches!(&meta, Meta::Path(path) if path.is_ident("skip_debug")) {
            crate::set_once(&mut skip_debug, true, meta.span(), "skip_debug")?;
            continue;
        }
        if matches!(&meta, Meta::Path(path) if path.is_ident("skip_default")) {
            crate::set_once(&mut skip_default, true, meta.span(), "skip_default")?;
            continue;
        }
        let value = path::declared(&meta)
            .ok_or_else(|| Error::new(meta.span(), "message accepts only harmos_path, skip_debug and skip_default"))??;
        crate::set_once(&mut explicit, value, meta.span(), path::KEY)?;
    }
    let skip_debug = skip_debug.unwrap_or(false);
    let skip_default = skip_default.unwrap_or(false);
    let parsed: syn::Item = syn::parse2(item)?;
    if let syn::Item::Enum(item) = parsed {
        return enumeration::expand(item, HarmosPath::resolve(explicit), skip_debug, skip_default);
    }
    let syn::Item::Struct(mut item) = parsed else {
        return Err(Error::new(parsed.span(), "message requires a struct or enum"));
    };
    if !matches!(item.fields, Fields::Named(_) | Fields::Unit) || !item.generics.params.is_empty() {
        return Err(Error::new(item.span(), "a message requires a non-generic struct with named fields (use {} for an empty message)"));
    }
    let harmos = HarmosPath::resolve(explicit);
    let row_metadata = crate::presentation::fields(&mut item.fields)?;
    let mut tags = BTreeMap::new();
    let mut fields = Vec::new();
    for field in &mut item.fields {
        // Macro-Forwarded Types Carry Invisible Delimiter Groups
        while let Type::Group(group) = &field.ty {
            field.ty = (*group.elem).clone();
        }
        let name = field.ident.as_ref().expect("named field").to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name);
        let number = tag(name);
        if let Some(previous) = tags.insert(number, name.to_owned()) {
            return Err(Error::new(
                field.span(),
                format!("message fields `{previous}` and `{name}` collide at protobuf tag {number}; rename a field explicitly"),
            ));
        }
        let (_, kind, cardinality) = encoding(&field.ty, &harmos)
            .map_err(|error| Error::new(field.span(), format!("field `{name}` of type `{}`: {error}", field.ty.to_token_stream())))?;
        fields.push(quote! {
            #harmos::schema::graph::Field {
                name: ::std::borrow::Cow::Borrowed(#name), tag: #number,
                kind: #kind, cardinality: #cardinality,
            }
        });
    }
    let postcard = postcard::declare(&syn::Item::Struct(item.clone()), &harmos);
    let declaration = composition::declare(&mut item, &harmos, skip_debug, skip_default)?;
    let gates = crate::operation::configuration(&item.attrs)?;
    let name = &item.ident;
    let schema = described(name, quote!(&[#(#fields),*]), quote!(), &harmos);
    let owned_fields = item.fields.iter().map(|field| {
        let name = field.ident.as_ref().expect("named field");
        quote! {
            let owned = #harmos::MemorySize::memory_size(&self.#name);
            if owned == ::std::primitive::usize::MAX {
                return ::std::primitive::usize::MAX;
            }
            let bytes = bytes.saturating_add(
                owned.saturating_sub(::std::mem::size_of_val(&self.#name))
            );
        }
    });
    Ok(quote! {
        #declaration
        #(#gates)*
        const _: () = {
            #postcard

            impl #harmos::MemorySize for #name {
                fn memory_size(&self) -> usize {
                    let bytes = ::std::mem::size_of::<Self>();

                    #(#owned_fields)*
                    bytes
                }
            }

            impl #harmos::RowMetadata for #name {
                const ROW_METADATA_JSON: &'static str = #row_metadata;
            }

            #schema
        };
    })
}

pub(crate) fn portable(item: &syn::Item, harmos: &HarmosPath) -> bool {
    let syn::Item::Struct(item) = item else {
        return false;
    };
    matches!(item.fields, Fields::Named(_))
        && item.generics.params.is_empty()
        && item.fields.iter().all(|field| common(&field.ty) && encoding(&field.ty, harmos).is_ok())
}

/* === Private Functions === */

fn described(name: &syn::Ident, fields: TokenStream, extra: TokenStream, harmos: &HarmosPath) -> TokenStream {
    quote! {
        impl #harmos::schema::Described for #name {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!(#name));
            const FIELDS: &'static [#harmos::schema::graph::Field<#harmos::schema::declaration::Definition>] = #fields;
            const MARKER: &'static [u8] = &#harmos::schema::declaration::encode::<{
                #harmos::schema::declaration::encoded_len(Self::NAME, Self::FIELDS)
            }>(Self::NAME, Self::FIELDS);
            #extra
        }
    }
}

fn tag(name: &str) -> u32 {
    let hash = name.bytes().fold(2_166_136_261_u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(16_777_619));
    let tag = 1 + hash % (536_870_911 - 1_000);
    if tag >= 19_000 { tag + 1_000 } else { tag }
}

fn encoding(ty: &Type, harmos: &HarmosPath) -> Result<(bool, TokenStream, TokenStream), Error> {
    let Type::Path(path) = ty else {
        return Err(unsupported(ty));
    };
    let segment = path.path.segments.last().ok_or_else(|| unsupported(ty))?;
    let name = segment.ident.to_string();
    // Box Changes Ownership Only: The Field Keeps The Plain Nested Message Wire Shape
    if matches!(name.as_str(), "Box" | "Rc" | "Arc") {
        let inner = boxed(ty).ok_or_else(|| unsupported_box(ty))?;
        return Ok((true, message_kind(inner, harmos), quote!(#harmos::schema::Cardinality::Required)));
    }
    if name == "HashMap" || name == "BTreeMap" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return Err(unsupported(ty));
        };
        let types = arguments
            .args
            .iter()
            .map(|argument| if let GenericArgument::Type(ty) = argument { Some(ty) } else { None })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| unsupported(ty))?;
        if types.len() != 2 && !(name == "HashMap" && types.len() == 3) {
            return Err(unsupported(ty));
        }
        let key = types[0];
        let value = types[1];
        let key_name = type_name(key);
        if !matches!(key_name.as_deref(), Some("bool" | "u32" | "u64" | "i32" | "i64" | "String")) {
            return Err(Error::new(key.span(), "map keys must be bool, u32, u64, i32, i64 or String"));
        }
        encoding(value, harmos)?;
        if matches!(type_name(value).as_deref(), Some("Option" | "HashMap" | "BTreeMap" | "Box" | "Rc" | "Arc"))
            || (type_name(value).as_deref() == Some("Vec") && !is_bytes(value))
        {
            return Err(Error::new(value.span(), "map values must be supported scalars, bytes or named messages"));
        }
        return Ok((
            false,
            quote!(#harmos::schema::graph::Kind::Message {
                definition: <#harmos::__private::MapEntry<#key, #value> as #harmos::schema::Described>::DEFINITION,
            }),
            quote!(#harmos::schema::Cardinality::Repeated),
        ));
    }
    if name == "Option" || name == "Vec" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return Err(unsupported(ty));
        };
        let Some(GenericArgument::Type(inner)) = arguments.args.first() else {
            return Err(unsupported(ty));
        };
        if arguments.args.len() != 1 {
            return Err(unsupported(ty));
        }
        let inner_name = match inner {
            Type::Path(path) => path.path.segments.last().map(|part| part.ident.to_string()),
            _ => None,
        };
        if name == "Vec" && inner_name.as_deref() == Some("u8") {
            return Ok((false, quote!(#harmos::schema::graph::Kind::Bytes), quote!(#harmos::schema::Cardinality::Required)));
        }
        if matches!(inner_name.as_deref(), Some("Box" | "Rc" | "Arc")) {
            return Err(unsupported_box(ty));
        }
        if matches!(inner_name.as_deref(), Some("Option" | "Vec" | "HashMap" | "BTreeMap")) {
            return Err(unsupported(ty));
        }
        let scalar = scalar(inner_name.as_deref().unwrap_or(""));
        let kind = scalar.map(|kind| quote!(#harmos::schema::graph::Kind::#kind)).unwrap_or_else(|| message_kind(inner, harmos));
        let cardinality =
            if name == "Option" { quote!(#harmos::schema::Cardinality::Optional) } else { quote!(#harmos::schema::Cardinality::Repeated) };
        return Ok((false, kind, cardinality));
    }
    if let Some(kind) = scalar(&name) {
        return Ok((false, quote!(#harmos::schema::graph::Kind::#kind), quote!(#harmos::schema::Cardinality::Required)));
    }
    if !matches!(segment.arguments, PathArguments::None)
        || matches!(name.as_str(), "u8" | "i8" | "u16" | "i16" | "usize" | "isize" | "u128" | "i128" | "char" | "HashMap" | "BTreeMap")
    {
        return Err(unsupported(ty));
    }
    Ok((true, message_kind(ty, harmos), quote!(#harmos::schema::Cardinality::Required)))
}

// Return The Nested Message Owned By A Direct `Box<T>` Field
fn boxed(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if path.qself.is_some() || segment.ident != "Box" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let (1, Some(GenericArgument::Type(inner))) = (arguments.args.len(), arguments.args.first()) else {
        return None;
    };
    let Type::Path(inner_path) = inner else {
        return None;
    };
    let inner_segment = inner_path.path.segments.last()?;
    let inner_name = inner_segment.ident.to_string();
    let reserved = matches!(
        inner_name.as_str(),
        "u8" | "i8"
            | "u16"
            | "i16"
            | "usize"
            | "isize"
            | "u128"
            | "i128"
            | "char"
            | "str"
            | "HashMap"
            | "BTreeMap"
            | "Box"
            | "Rc"
            | "Arc"
            | "Option"
            | "Vec"
    );
    let nested =
        inner_path.qself.is_none() && matches!(inner_segment.arguments, PathArguments::None) && scalar(&inner_name).is_none() && !reserved;
    nested.then_some(inner)
}

fn message_kind(ty: &Type, harmos: &HarmosPath) -> TokenStream {
    quote!(#harmos::schema::graph::Kind::Message {
        definition: <#ty as #harmos::schema::Described>::DEFINITION,
    })
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else { return None };
    path.path.segments.last().map(|segment| segment.ident.to_string())
}

fn is_bytes(ty: &Type) -> bool {
    let Type::Path(path) = ty else { return false };
    let Some(segment) = path.path.segments.last() else { return false };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else { return false };
    arguments.args.len() == 1
        && matches!(arguments.args.first(), Some(GenericArgument::Type(inner)) if type_name(inner).as_deref() == Some("u8"))
}

fn scalar(name: &str) -> Option<TokenStream> {
    Some(match name {
        "bool" => quote!(Bool),
        "u32" => quote!(U32),
        "u64" => quote!(U64),
        "i32" => quote!(I32),
        "i64" => quote!(I64),
        "f32" => quote!(F32),
        "f64" => quote!(F64),
        "String" => quote!(String),
        _ => return None,
    })
}

fn unsupported(ty: &Type) -> Error {
    Error::new(
        ty.span(),
        "unsupported message field: use bool/u32/u64/i32/i64/f32/f64/String, Vec<u8> bytes, a nested message, Box<T> of a nested message, Option<T>, or Vec<T> of supported scalars/messages; HashMap/BTreeMap with scalar keys and scalar/bytes/message values are supported; aliases and recursive containers require an explicit message shape",
    )
}

fn unsupported_box(ty: &Type) -> Error {
    Error::new(
        ty.span(),
        "unsupported boxed message field: use Box<T> directly as the field type, where T is a nested message; Box of scalars, strings, bytes or containers, Box<Box<T>>, Option<Box<T>>, Vec<Box<T>>, Rc<T> and Arc<T> are unsupported",
    )
}

fn common(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    let name = segment.ident.to_string();
    if scalar(&name).is_some() || name == "u8" {
        return true;
    }
    if name == "HashMap" || name == "BTreeMap" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else { return false };
        return arguments.args.iter().take(2).all(|arg| matches!(arg, GenericArgument::Type(inner) if common(inner)));
    }
    if name == "Vec" || name == "Option" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return false;
        };
        return arguments.args.len() == 1 && matches!(arguments.args.first(), Some(GenericArgument::Type(inner)) if common(inner));
    }
    false
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{Delimiter, Group};

    #[test]
    fn map_field_shapes() {
        for ty in [quote!(HashMap<String, u64>), quote!(BTreeMap<bool, Vec<u8>>), quote!(std::collections::HashMap<i32, Row>)] {
            assert!(expand(quote!(), quote!(struct Holder { values: #ty })).is_ok());
        }
        for ty in [
            quote!(HashMap<f64, String>),
            quote!(BTreeMap<String, Vec<String>>),
            quote!(HashMap<String, Option<u32>>),
            quote!(HashMap<String, BTreeMap<String, String>>),
            quote!(HashMap<String>),
            quote!(Option<HashMap<String, String>>),
            quote!(Vec<BTreeMap<String, String>>),
        ] {
            assert!(expand(quote!(), quote!(struct Holder { values: #ty })).is_err());
        }
    }

    #[test]
    fn forwarded_field_shapes() {
        let supported = Group::new(Delimiter::None, quote!(Option<i64>));
        let supported = TokenStream::from(proc_macro2::TokenTree::Group(supported));
        let expanded = expand(quote!(), quote!(struct Row { value: #supported }));
        assert!(expanded.is_ok());

        let unsupported = Group::new(Delimiter::None, quote!(Option<Vec<String>>));
        let unsupported = TokenStream::from(proc_macro2::TokenTree::Group(unsupported));
        let expanded = expand(quote!(), quote!(struct Row { value: #unsupported }));
        let error = expanded.expect_err("forwarding must not admit recursive containers");
        assert!(error.to_string().contains("unsupported message field"));
    }
}
