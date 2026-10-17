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
use std::collections::BTreeSet;

// Third Party Imports
use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Error, Expr, ItemStruct, Type};

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
// The request type declares its operation: the attribute parses the metadata
// and expands to the SDK's `declare!`, the one writer of the declaration impl.
pub(crate) fn structure(attribute: TokenStream, item: ItemStruct, sdk: &HarmosPath) -> Result<TokenStream, Error> {
    if !item.generics.params.is_empty() {
        return Err(Error::new_spanned(&item.generics, "an operation request type cannot be generic"));
    }
    let attribute: Attribute = syn::parse_quote!(#[operation(#attribute)]);
    let mut seen = BTreeSet::new();
    let mut response = None::<Type>;
    let mut input = None::<Type>;
    let mut output = None::<Type>;
    let mut id = None::<Expr>;
    let mut policy = None::<Expr>;
    let mut version = None::<Expr>;
    let mut description = None::<Expr>;
    let mut visibility = None::<Expr>;
    let mut replay = None::<Expr>;
    let mut grant = None::<Expr>;
    let mut key = None::<Expr>;
    let mut grpc = None;
    attribute.parse_nested_meta(|meta| {
        let Some(name) = meta.path.get_ident() else {
            return Err(meta.error("expected an operation metadata field"));
        };
        let name = name.to_string();
        if !seen.insert(name.clone()) {
            return Err(meta.error("operation metadata fields cannot repeat"));
        }
        match name.as_str() {
            "request" => {
                return Err(meta.error("the operation is its request type: put the attribute on the request struct"));
            }
            "response" => response = Some(meta.value()?.parse()?),
            "input" => input = Some(meta.value()?.parse()?),
            "output" => output = Some(meta.value()?.parse()?),
            "id" => id = Some(meta.value()?.parse()?),
            "policy" => policy = Some(meta.value()?.parse()?),
            "version" => version = Some(meta.value()?.parse()?),
            "description" => description = Some(meta.value()?.parse()?),
            "visibility" => visibility = Some(meta.value()?.parse()?),
            "replay" => replay = Some(meta.value()?.parse()?),
            "grant" => grant = Some(meta.value()?.parse()?),
            "key" => key = Some(meta.value()?.parse()?),
            "grpc" => grpc = Some(grpc_binding(meta)?),
            _ => return Err(meta.error("unknown operation metadata field")),
        }
        Ok(())
    })?;
    let missing =
        |field| Error::new_spanned(&attribute, format!("operation requires `{field}` (response, version and policy are required)"));
    let response = response.ok_or_else(|| missing("response"))?;
    let version = version.ok_or_else(|| missing("version"))?;
    let policy = policy.ok_or_else(|| missing("policy"))?;
    let name = &item.ident;
    let derived = name.to_string().trim_start_matches("r#").to_snake_case();
    let id = id.unwrap_or_else(|| syn::parse_quote!(#derived));
    let input = input.map(|item| quote!(, input: #item));
    let output = output.map(|item| quote!(, output: #item));
    let description = description.map(|value| quote!(, description: #value));
    let visibility = visibility.map(|value| quote!(, visibility: #value));
    let replay = replay.map(|value| quote!(, replay: #value));
    let grpc = grpc.map(|(package, service, method)| quote!(, grpc: (#package, #service, #method)));
    let grant = grant.map(|value| quote!(, grant: #value));
    let key = key.map(|value| quote!(, key: #value));
    Ok(quote! {
        #item
        #sdk::__private::declare!(#name {
            id: #id,
            version: #version,
            policy: #policy,
            response: #response
            #input #output #description #visibility #replay #grpc #grant #key
        });
    })
}

/* === Private Functions === */
fn grpc_binding(meta: syn::meta::ParseNestedMeta<'_>) -> Result<(syn::LitStr, syn::LitStr, syn::LitStr), Error> {
    let mut values = std::collections::BTreeMap::new();
    meta.parse_nested_meta(|field| {
        let name = field.path.get_ident().map(ToString::to_string).ok_or_else(|| field.error("expected package, service or method"))?;
        if !matches!(name.as_str(), "package" | "service" | "method") {
            return Err(field.error("expected package, service or method"));
        }
        let value: syn::LitStr = field.value()?.parse()?;
        if values.insert(name, value).is_some() {
            return Err(field.error("gRPC binding fields cannot repeat"));
        }
        Ok(())
    })?;
    let mut take = |name| values.remove(name).ok_or_else(|| meta.error("grpc requires package, service and method"));
    let package = take("package")?;
    let service = take("service")?;
    let method = take("method")?;
    for (name, value) in [("package", &package), ("service", &service), ("method", &method)] {
        let text = value.value();
        let valid = |value: &str| {
            let mut bytes = value.bytes();
            bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        };
        if text.len() > 255
            || (name == "package" && text.starts_with("grpc."))
            || !(if name == "package" { text.split('.').all(valid) } else { valid(&text) })
        {
            return Err(Error::new_spanned(value, "invalid or reserved gRPC binding name"));
        }
    }
    Ok((package, service, method))
}

/* === Tests === */
