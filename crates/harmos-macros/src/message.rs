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
mod decoding;
mod enumeration;
mod postcard;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
// Third Party Imports
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Error, Fields, GenericArgument, Meta, PathArguments, Token, Type, parse::Parser,
    punctuated::Punctuated, spanned::Spanned,
};
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
    for meta in Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attributes)? {
        if matches!(&meta, Meta::Path(path) if path.is_ident("skip_debug")) {
            crate::artifact::set_once(&mut skip_debug, true, meta.span(), "skip_debug")?;
            continue;
        }
        let value = path::declared(&meta).ok_or_else(|| {
            Error::new(
                meta.span(),
                "message accepts only harmos_path and skip_debug",
            )
        })??;
        crate::artifact::set_once(&mut explicit, value, meta.span(), path::KEY)?;
    }
    let skip_debug = skip_debug.unwrap_or(false);
    let parsed: syn::Item = syn::parse2(item)?;
    if let syn::Item::Enum(item) = parsed {
        return enumeration::expand(item, HarmosPath::resolve(explicit), skip_debug);
    }
    let syn::Item::Struct(mut item) = parsed else {
        return Err(Error::new(
            parsed.span(),
            "message requires a struct or enum",
        ));
    };
    if !matches!(item.fields, Fields::Named(_)) || !item.generics.params.is_empty() {
        return Err(Error::new(
            item.span(),
            "a message requires a non-generic struct with named fields (use {} for an empty message)",
        ));
    }
    if item
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("prost"))
    {
        return Err(Error::new(
            item.span(),
            "message owns protobuf attributes and tags",
        ));
    }
    let harmos = HarmosPath::resolve(explicit);
    let row_metadata = crate::presentation::fields(&mut item.fields)?;
    let mut tags = BTreeMap::new();
    let mut fields = Vec::new();
    for field in &mut item.fields {
        if field
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("prost"))
        {
            return Err(Error::new(
                field.span(),
                "remove prost attributes: message derives tags from field names",
            ));
        }
        let name = field.ident.as_ref().expect("named field").to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name);
        let number = tag(name);
        if let Some(previous) = tags.insert(number, name.to_owned()) {
            return Err(Error::new(
                field.span(),
                format!(
                    "message fields `{previous}` and `{name}` collide at protobuf tag {number}; rename a field explicitly"
                ),
            ));
        }
        let (encoding, kind, cardinality) = encoding(&field.ty, &harmos)?;
        fields.push(quote! {
            #harmos::schema::Field {
                name: ::std::borrow::Cow::Borrowed(#name), tag: #number,
                kind: #kind, cardinality: #cardinality,
            }
        });
        let number = syn::LitStr::new(&number.to_string(), field.span());
        field
            .attrs
            .push(syn::parse_quote!(#[prost(#encoding, tag = #number)]));
    }
    let postcard = postcard::declare(&syn::Item::Struct(item.clone()), &harmos);
    let declaration = composition::declare(&mut item, &harmos, skip_debug)?;
    let name = &item.ident;
    let fields = fields.into_iter();
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

        impl #harmos::schema::Described for #name {
            const FIELDS: &'static [#harmos::schema::Field] = &[#(#fields),*];
        }
    })
}

pub(crate) fn portable(item: &syn::Item, harmos: &HarmosPath) -> bool {
    let syn::Item::Struct(item) = item else {
        return false;
    };
    matches!(item.fields, Fields::Named(_))
        && item.generics.params.is_empty()
        && item
            .fields
            .iter()
            .all(|field| common(&field.ty) && encoding(&field.ty, harmos).is_ok())
}

/* === Private Functions === */
fn tag(name: &str) -> u32 {
    let hash = name.bytes().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    });
    let tag = 1 + hash % (536_870_911 - 1_000);
    if tag >= 19_000 { tag + 1_000 } else { tag }
}

fn encoding(
    ty: &Type,
    harmos: &HarmosPath,
) -> Result<(TokenStream, TokenStream, TokenStream), Error> {
    let Type::Path(path) = ty else {
        return Err(unsupported(ty));
    };
    let segment = path.path.segments.last().ok_or_else(|| unsupported(ty))?;
    let name = segment.ident.to_string();
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
            return Ok((
                quote!(bytes = "vec"),
                quote!(#harmos::schema::Kind::Bytes),
                quote!(#harmos::schema::Cardinality::Required),
            ));
        }
        if matches!(inner_name.as_deref(), Some("Option" | "Vec")) {
            return Err(unsupported(ty));
        }
        let scalar = scalar(inner_name.as_deref().unwrap_or(""));
        let is_scalar = scalar.is_some();
        let (wire, kind) = scalar
            .map(|(wire, kind)| (wire, quote!(#harmos::schema::Kind::#kind)))
            .unwrap_or_else(|| (quote!(message), message_kind(inner, harmos)));
        return Ok(if name == "Option" {
            (
                quote!(#wire, optional),
                kind,
                quote!(#harmos::schema::Cardinality::Optional),
            )
        } else if is_scalar && inner_name.as_deref() != Some("String") {
            (
                quote!(#wire, repeated, packed = "true"),
                kind,
                quote!(#harmos::schema::Cardinality::Repeated),
            )
        } else {
            (
                quote!(#wire, repeated),
                kind,
                quote!(#harmos::schema::Cardinality::Repeated),
            )
        });
    }
    if let Some((wire, kind)) = scalar(&name) {
        return Ok((
            wire,
            quote!(#harmos::schema::Kind::#kind),
            quote!(#harmos::schema::Cardinality::Required),
        ));
    }
    if !matches!(segment.arguments, PathArguments::None)
        || matches!(
            name.as_str(),
            "u8" | "i8"
                | "u16"
                | "i16"
                | "usize"
                | "isize"
                | "u128"
                | "i128"
                | "char"
                | "HashMap"
                | "BTreeMap"
                | "Box"
        )
    {
        return Err(unsupported(ty));
    }
    Ok((
        quote!(message, required),
        message_kind(ty, harmos),
        quote!(#harmos::schema::Cardinality::Required),
    ))
}

fn message_kind(ty: &Type, harmos: &HarmosPath) -> TokenStream {
    quote!(#harmos::schema::Kind::Message {
        fields: ::std::borrow::Cow::Borrowed(<#ty as #harmos::schema::Described>::FIELDS),
    })
}

fn scalar(name: &str) -> Option<(TokenStream, TokenStream)> {
    Some(match name {
        "bool" => (quote!(bool), quote!(Bool)),
        "u32" => (quote!(uint32), quote!(U32)),
        "u64" => (quote!(uint64), quote!(U64)),
        "i32" => (quote!(int32), quote!(I32)),
        "i64" => (quote!(int64), quote!(I64)),
        "f32" => (quote!(float), quote!(F32)),
        "f64" => (quote!(double), quote!(F64)),
        "String" => (quote!(string), quote!(String)),
        _ => return None,
    })
}

fn unsupported(ty: &Type) -> Error {
    Error::new(
        ty.span(),
        "unsupported message field: use bool/u32/u64/i32/i64/f32/f64/String, Vec<u8> bytes, a nested message, Option<T>, or Vec<T> of supported scalars/messages; maps, aliases and recursive containers require an explicit message shape",
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
    if name == "Vec" || name == "Option" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return false;
        };
        return arguments.args.len() == 1
            && matches!(arguments.args.first(), Some(GenericArgument::Type(inner)) if common(inner));
    }
    false
}

/* === Tests === */
