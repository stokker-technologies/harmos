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
use quote::{ToTokens, format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Fields, Ident, Item, LitInt, LitStr, Meta, Path, Token, Type};
// Local Imports
use crate::path::{self, HarmosPath};
use crate::set_once;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Declaration {
    id: Option<LitStr>,
    presentation: crate::presentation::Presentation,
    window: Option<LitInt>,
    row: Option<Type>,
    harmos: HarmosPath,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn stamp(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, &item) {
        Ok(stamped) => stamped,
        Err(refused) => {
            let refusal = refused.to_compile_error();

            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */

fn expand(attribute: TokenStream, item: &TokenStream) -> Result<TokenStream, Error> {
    let mut declared = declaration(attribute)?;
    let mut parsed: Item = syn::parse2(item.clone())?;
    declared.presentation.default_label(&named(&parsed)?.to_string());
    let metadata =
        declared.presentation.metadata(declared.id.as_ref().map(|id| id.value()).as_deref().unwrap_or(""), attributes(&parsed)).to_string();
    if let Some(row) = &declared.row {
        let Item::Struct(marker) = &parsed else {
            return Err(Error::new(parsed.span(), "a named stream uses a unit struct"));
        };
        if !matches!(marker.fields, Fields::Unit) || !marker.generics.params.is_empty() {
            return Err(Error::new(marker.span(), "a named stream uses a non-generic unit struct"));
        }
        let id = declared.id.as_ref().ok_or_else(|| Error::new(marker.span(), "a named stream requires id and window"))?;
        let window = declared.window.as_ref().ok_or_else(|| Error::new(marker.span(), "a named stream requires id and window"))?;
        let name = &marker.ident;
        let harmos = &declared.harmos;
        return Ok(quote! {
            #item
            const _: () = assert!(#harmos::__private::stream_identity(#id),
                "a stream id must be a filesystem-safe dotted name with non-keyword ASCII segments; owner must match [a-z][a-z0-9_]*");
            impl #harmos::StreamSpec for #name {
                type Row = #row;
                const ID: &'static str = #id;
                const WINDOW: usize = #window;
                const METADATA_JSON: &'static str = #metadata;
                const ROW_METADATA_JSON: &'static str = <#row as #harmos::RowMetadata>::ROW_METADATA_JSON;
            }
        });
    }

    let mut presentation_shape = parsed.clone();
    let row_metadata =
        if let Item::Struct(shape) = &mut presentation_shape { crate::presentation::fields(&mut shape.fields)? } else { "[]".to_owned() };
    let explicit_message = match &parsed {
        Item::Struct(shape) => shape
            .attrs
            .iter()
            .any(|attr| attr.path().segments.last().is_some_and(|part| part.ident == "message") || attr.path().is_ident("prost")),
        _ => false,
    };
    let portable = explicit_message || crate::message::portable(&parsed, &declared.harmos);
    let mut item = parsed.to_token_stream();
    if matches!(&parsed, Item::Struct(shape) if matches!(shape.fields, Fields::Named(_)) && shape.generics.params.is_empty()) {
        let Item::Struct(shape) = &mut parsed else { unreachable!() };
        if !explicit_message {
            let harmos = &declared.harmos;
            let derives = shape
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("derive"))
                .map(|attr| attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated))
                .collect::<Result<Vec<_>, _>>()?;
            let has = |name: &str| derives.iter().flatten().any(|path| path.segments.last().is_some_and(|segment| segment.ident == name));
            let mut added = Vec::<TokenStream>::new();
            if !has("Clone") {
                added.push(quote!(Clone));
            }
            if portable && !has("PartialEq") {
                added.push(quote!(PartialEq));
            }
            if !has("Serialize") {
                added.push(quote!(#harmos::__private::serde::Serialize));
            }
            if !has("Deserialize") {
                added.push(quote!(#harmos::__private::serde::Deserialize));
            }
            if !added.is_empty() {
                shape.attrs.push(syn::parse_quote!(#[derive(#(#added),*)]));
            }
            if !has("Serialize") && !has("Deserialize") {
                let serde_path = LitStr::new(&format!("{}::__private::serde", harmos.to_token_stream()).replace(' ', ""), shape.span());
                shape.attrs.push(syn::parse_quote!(#[serde(crate = #serde_path)]));
            }
            let mut attributes = Vec::new();
            for attribute in std::mem::take(&mut shape.attrs) {
                if attribute.path().is_ident("derive") {
                    let paths = attribute.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
                    let paths: Vec<_> = paths
                        .into_iter()
                        .filter(|path| {
                            !portable || !path.segments.last().is_some_and(|part| part.ident == "Debug" || part.ident == "Default")
                        })
                        .collect();
                    if !paths.is_empty() {
                        attributes.push(syn::parse_quote!(#[derive(#(#paths),*)]));
                    }
                } else {
                    attributes.push(attribute);
                }
            }
            shape.attrs = attributes;
            if portable {
                item = crate::message::expand(quote!(harmos_path = #harmos), parsed.to_token_stream())?;
            } else {
                crate::presentation::fields(&mut shape.fields)?;
                let name = &shape.ident;
                item = quote! {
                    #shape
                    impl #harmos::RowMetadata for #name {
                        const ROW_METADATA_JSON: &'static str = #row_metadata;
                    }
                };
            }
        }
    } else {
        item = presentation_shape.to_token_stream();
    }
    if !portable {
        let schema = crate::message::shape(&parsed, &declared.harmos)?;
        item = quote!(#item #schema);
    }
    let row_metadata = if portable {
        let harmos = &declared.harmos;
        quote!(<Self as #harmos::RowMetadata>::ROW_METADATA_JSON)
    } else {
        quote!(#row_metadata)
    };
    let name = named(&parsed)?;
    let id = declared.id.as_ref().ok_or_else(|| Error::new(parsed.span(), "a stream requires id and window"))?;
    let Some(window) = &declared.window else {
        return Err(Error::new(id.span(), "a current stream row declares `id = \"…\"` and `window = …`"));
    };
    let encoded_id: String = id.value().bytes().map(|byte| format!("{byte:02x}")).collect();
    let id_guard = format_ident!("__HARMOS_STREAM_ID_{}", encoded_id);
    let harmos = &declared.harmos;
    Ok(quote! {
        #item

        const _: () = assert!(#harmos::__private::stream_identity(#id),
            "a stream id must be a filesystem-safe dotted name with non-keyword ASCII segments; owner must match [a-z][a-z0-9_]*");


        #[automatically_derived]
        impl #harmos::StreamSpec for #name {
            type Row = Self;
            const ID: &'static str = #id;
            const WINDOW: usize = #window;
            const METADATA_JSON: &'static str = #metadata;
            const ROW_METADATA_JSON: &'static str = #row_metadata;
        }

        #[automatically_derived]
        impl #harmos::__private::StreamRow for #name {
            const ID: &'static str = #id;
            const WINDOW: usize = #window;
            fn metadata() -> #harmos::Metadata { <Self as #harmos::StreamSpec>::metadata() }
            fn row_metadata() -> Vec<#harmos::StreamField> { <Self as #harmos::StreamSpec>::row_metadata() }
        }

        impl #name {
            pub const ID: &'static str = #id;


            pub const WINDOW: usize = #window;


            pub fn definition() -> #harmos::streams::Definition {
                #harmos::__private::stream_definition::<Self>(#id, #window)
            }
        }

        const _: () = {
            fn stream_rows_are_serde<T>()
            where
                T: #harmos::__private::serde::Serialize + #harmos::__private::serde::de::DeserializeOwned,
            {
            }

            fn this_stream_row_is_serde() {
                stream_rows_are_serde::<#name>();
            }
        };


        const #id_guard: () = ();
    })
}

fn attributes(item: &Item) -> &[syn::Attribute] {
    match item {
        Item::Struct(shape) => &shape.attrs,
        Item::Enum(shape) => &shape.attrs,
        Item::Type(shape) => &shape.attrs,
        _ => &[],
    }
}

fn declaration(attribute: TokenStream) -> Result<Declaration, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut id = None;
    let mut presentation = crate::presentation::Presentation::default();
    let mut window = None;
    let mut row = None;
    let mut harmos_path: Option<Path> = None;

    for argument in &arguments {
        if presentation.parse(argument, false)? {
            continue;
        }
        if let Some(parsed) = path::declared(argument) {
            set_once(&mut harmos_path, parsed?, argument.span(), path::KEY)?;
            continue;
        }
        match argument {
            Meta::NameValue(named) if named.path.is_ident("id") => {
                set_once(&mut id, syn::parse2::<LitStr>(named.value.to_token_stream())?, named.span(), "id")?;
            }
            Meta::NameValue(named) if named.path.is_ident("window") => {
                set_once(&mut window, syn::parse2::<LitInt>(named.value.to_token_stream())?, named.span(), "window")?;
            }
            Meta::NameValue(named) if named.path.is_ident("row") => {
                set_once(&mut row, syn::parse2::<Type>(named.value.to_token_stream())?, named.span(), "row")?;
            }
            other => {
                return Err(Error::new(other.span(), "a stream declares `id = \"…\"`, `window = …`, and optionally `harmos_path = …`"));
            }
        }
    }

    if let Some(id) = &id {
        validate_id(id)?;
    }
    if let Some(window) = &window {
        positive_usize(window, "a stream window must retain at least 1 row")?;
    }
    Ok(Declaration {
        id,
        presentation,
        window,
        row,
        harmos: HarmosPath::resolve(harmos_path),
    })
}

fn positive_usize(literal: &LitInt, message: &str) -> Result<(), Error> {
    if !matches!(literal.suffix(), "" | "usize") {
        return Err(Error::new(literal.span(), "a stream window is an unsuffixed integer or `usize`"));
    }
    let value = literal.base10_parse::<usize>().map_err(|error| Error::new(literal.span(), error))?;
    if value == 0 {
        return Err(Error::new(literal.span(), message));
    }

    Ok(())
}

fn validate_id(id: &LitStr) -> Result<(), Error> {
    let value = id.value();
    let safe = value.split('.').all(|segment| !python_keyword(segment) && !filesystem_reserved(segment));
    if !safe {
        return Err(Error::new(
            id.span(),
            "a stream id must be a filesystem-safe dotted name with non-keyword ASCII segments; owner must match [a-z][a-z0-9_]*",
        ));
    }
    Ok(())
}

fn filesystem_reserved(value: &str) -> bool {
    let uppercase = value.to_ascii_uppercase();

    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || uppercase.strip_prefix("COM").is_some_and(|number| matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        || uppercase.strip_prefix("LPT").is_some_and(|number| matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

fn python_keyword(value: &str) -> bool {
    matches!(
        value,
        "False"
            | "None"
            | "True"
            | "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

fn named(item: &Item) -> Result<&Ident, Error> {
    match item {
        Item::Struct(shape) => Ok(&shape.ident),
        Item::Enum(shape) => Ok(&shape.ident),
        other => Err(Error::new(other.span(), "a stream attribute belongs on the struct or enum the row is")),
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_series_and_batch_policies_are_refused() {
        let row = quote!(
            struct Sample {
                value: u64,
            }
        );
        for policy in [quote!(track = (value,)), quote!(axis = value), quote!(batch(samples = 3))] {
            let declaration = quote!(id = "sample", window = 8, #policy);
            assert!(expand(declaration, &row).is_err());
        }
    }
}
