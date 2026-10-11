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
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Fields, ItemStruct, LitInt, LitStr, Meta, Token, Type};
// Local Imports
use crate::artifact::set_once;
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) fn expand(attributes: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let declaration: ItemStruct = syn::parse2(item)?;
    if !declaration.generics.params.is_empty() || !matches!(declaration.fields, Fields::Unit) {
        return Err(Error::new(
            declaration.span(),
            "a portable listener is a nongeneric unit descriptor struct",
        ));
    }
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attributes)?;
    let mut id = None;
    let mut record = None;
    let mut version = None;
    let mut row = None;
    let mut stream = None;
    let mut callback = None;
    let mut facade = None;
    for argument in arguments {
        if let Some(value) = path::declared(&argument) {
            set_once(&mut facade, value?, argument.span(), path::KEY)?;
            continue;
        }
        let Meta::NameValue(named) = argument else {
            return Err(Error::new(
                argument.span(),
                "listener declarations use id, record/version/row or stream, callback, and harmos_path",
            ));
        };
        let tokens = named.value.to_token_stream();
        if named.path.is_ident("id") || named.path.is_ident("record") {
            let value: LitStr = syn::parse2(tokens)?;
            if value.value().trim().is_empty() || value.value().chars().any(char::is_control) {
                return Err(Error::new(
                    value.span(),
                    "listener and source identities must be nonempty without control characters",
                ));
            }
            if named.path.is_ident("id") {
                set_once(&mut id, value, named.span(), "id")?;
            } else {
                set_once(&mut record, value, named.span(), "record")?;
            }
        } else if named.path.is_ident("version") {
            let value: LitInt = syn::parse2(tokens)?;
            if value.base10_parse::<u32>()? == 0 {
                return Err(Error::new(
                    value.span(),
                    "record semantic version must be positive",
                ));
            }
            set_once(&mut version, value, named.span(), "version")?;
        } else if named.path.is_ident("row")
            || named.path.is_ident("stream")
            || named.path.is_ident("callback")
        {
            let value: Type = syn::parse2(tokens)?;
            if named.path.is_ident("row") {
                set_once(&mut row, value, named.span(), "row")?;
            } else if named.path.is_ident("stream") {
                set_once(&mut stream, value, named.span(), "stream")?;
            } else {
                set_once(&mut callback, value, named.span(), "callback")?;
            }
        } else {
            return Err(Error::new(
                named.span(),
                "listener declarations use id, record/version/row or stream, callback, and harmos_path",
            ));
        }
    }
    let span = declaration.span();
    let id = id.ok_or_else(|| Error::new(span, "a portable listener requires id"))?;
    let callback = callback.ok_or_else(|| {
        Error::new(
            span,
            "a portable listener requires its exact callback operation type",
        )
    })?;
    let harmos = HarmosPath::resolve(facade);
    let (source, row) = match (record, version, row, stream) {
        (Some(record), Some(version), Some(row), None) => {
            let source = quote!(#harmos::listeners::Source::Record {
                id: ::std::borrow::Cow::Borrowed(#record), version: #version,
                fields: ::std::borrow::Cow::Borrowed(<#row as #harmos::schema::Described>::FIELDS),
            });
            (source, quote!(#row))
        }
        (None, None, None, Some(stream)) => {
            let row = quote!(<#stream as #harmos::StreamSpec>::Row);
            let source = quote!(#harmos::listeners::Source::Stream {
                id: ::std::borrow::Cow::Borrowed(<#stream as #harmos::StreamSpec>::ID),
                schema_ordinal: <#stream as #harmos::StreamSpec>::VERSION,
                fields: ::std::borrow::Cow::Borrowed(<#row as #harmos::schema::Described>::FIELDS),
            });
            (source, row)
        }
        _ => {
            return Err(Error::new(
                span,
                "select exactly record/version/row or a typed stream declaration",
            ));
        }
    };
    let name = &declaration.ident;
    Ok(quote! {
        #declaration
        impl #harmos::catalog::Member<#name> for #harmos::listeners::Declarations {
            const ENTRY: #harmos::listeners::Entry = #harmos::listeners::Entry {
                id: #id, source: #source,
                callback: #harmos::operations::WireDeclaration::of::<
                    <#callback as #harmos::operations::Declares>::Declaration
                >(),
            };
        }
        const _: () = {
            fn require_callback<O: #harmos::operations::WireOperation<Request = #harmos::listeners::Delivery<#row>, Response = ()>>() {}
            let _ = require_callback::<<#callback as #harmos::operations::Declares>::Declaration>;
        };
    })
}

/* === Private Functions === */

/* === Tests === */
