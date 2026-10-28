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
use syn::parse::{Parse, Parser};
use syn::punctuated::Punctuated;
use syn::{Attribute, Error, Meta, Token, Type};
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

pub(crate) fn declare(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(attribute, item, false, false).unwrap_or_else(Error::into_compile_error)
}

pub(crate) fn declare_runtime(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(attribute, item, true, false).unwrap_or_else(Error::into_compile_error)
}

pub(crate) fn declare_domain(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(attribute, item, false, true).unwrap_or_else(Error::into_compile_error)
}

/* === Private Functions === */

fn arguments(input: syn::parse::ParseStream<'_>) -> Result<Punctuated<Meta, Token![,]>, Error> {
    input.parse_terminated(argument, Token![,])
}

fn argument(input: syn::parse::ParseStream<'_>) -> Result<Meta, Error> {
    let fork = input.fork();
    let path: syn::Path = fork.parse()?;
    if ["response", "input", "output"].iter().any(|name| path.is_ident(name)) {
        let path = input.parse()?;
        let eq_token = input.parse()?;
        let value: Type = input.parse()?;
        return Ok(Meta::NameValue(syn::MetaNameValue {
            path,
            eq_token,
            value: syn::Expr::Verbatim(quote!(#value)),
        }));
    }
    if path.is_ident("resources") || path.is_ident("operations") {
        let path = input.parse()?;
        let eq_token = input.parse()?;
        let content;
        syn::bracketed!(content in input);
        let types = content.parse_terminated(Type::parse, Token![,])?;
        return Ok(Meta::NameValue(syn::MetaNameValue {
            path,
            eq_token,
            value: syn::Expr::Verbatim(quote!([#types])),
        }));
    }
    input.parse()
}

fn expand(attribute: TokenStream, item: TokenStream, runtime: bool, domain: bool) -> Result<TokenStream, Error> {
    let declaration: syn::ItemStruct = syn::parse2(item).map_err(|error| {
        Error::new(
            error.span(),
            "operation belongs on the implementation struct; implement Operation explicitly under cfg(feature = \"runtime\")",
        )
    })?;
    let mut explicit = None;
    let mut metadata = Vec::new();
    for argument in arguments.parse2(attribute)? {
        if let Some(value) = path::declared(&argument) {
            if explicit.replace(value?).is_some() {
                return Err(Error::new_spanned(argument, "harmos_path may be declared only once"));
            }
        } else {
            metadata.push(argument);
        }
    }
    let sdk = explicit.map(|path| HarmosPath::resolve(Some(path))).unwrap_or_else(HarmosPath::catalog);
    let sdk = if runtime || domain { sdk.runtime_enabled() } else { sdk };
    crate::catalog::operation::structure(quote!(#(#metadata),*), declaration, &sdk, !domain)
}

pub(crate) fn configuration(attributes: &[Attribute]) -> Result<Vec<Attribute>, Error> {
    fn select(meta: &Meta) -> Result<Option<Meta>, Error> {
        if meta.path().is_ident("cfg") {
            return Ok(Some(meta.clone()));
        }
        let Meta::List(list) = meta else { return Ok(None) };
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

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::declare_domain;
    use quote::quote;

    #[test]
    fn portable_domain_operation() {
        let expanded = declare_domain(
            quote!(resources = [crate::Owner], id = "domain.read", policy = crate::POLICY),
            quote!(
                struct Read;
            ),
        )
        .to_string();

        assert!(expanded.contains("OperationDeclaration"));
        assert!(!expanded.contains("Dependent"));
        assert!(!expanded.contains("Requires"));
    }
}
