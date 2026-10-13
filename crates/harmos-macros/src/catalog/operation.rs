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
use syn::{Attribute, Error, Expr, Fields, ItemStruct, Type};

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
pub(crate) fn structure(
    attribute: TokenStream,
    item: ItemStruct,
    sdk: &HarmosPath,
) -> Result<TokenStream, Error> {
    if !matches!(item.fields, Fields::Unit) || !item.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &item,
            "an operation declaration requires a non-generic unit struct",
        ));
    }
    let attribute: Attribute = syn::parse_quote!(#[operation(#attribute)]);
    let mut seen = BTreeSet::new();
    let mut request = None::<Type>;
    let mut response = None::<Type>;
    let mut input = None::<Type>;
    let mut output = None::<Type>;
    let mut id = None::<Expr>;
    let mut policy = None::<Expr>;
    let mut constants = Vec::new();
    let mut key = None::<Expr>;
    let mut version = None::<Expr>;
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
            "request" => request = Some(meta.value()?.parse()?),
            "response" => response = Some(meta.value()?.parse()?),
            "input" => input = Some(meta.value()?.parse()?),
            "output" => output = Some(meta.value()?.parse()?),
            "id" => id = Some(meta.value()?.parse()?),
            "policy" => policy = Some(meta.value()?.parse()?),
            "key" => key = Some(meta.value()?.parse()?),
            "version" => version = Some(meta.value()?.parse()?),
            "grpc" => grpc = Some(grpc_binding(meta, sdk)?),
            "description" => {
                let value: Expr = meta.value()?.parse()?;
                constants.push(quote!(const DESCRIPTION: &'static str = #value;));
            }
            "visibility" => {
                let value: Expr = meta.value()?.parse()?;
                constants.push(quote!(const VISIBILITY: #sdk::operations::Visibility = #value;));
            }
            "replay" => {
                let value: Expr = meta.value()?.parse()?;
                constants.push(quote!(const REPLAY: #sdk::operations::ReplaySafety = #value;));
            }
            _ => return Err(meta.error("unknown operation metadata field")),
        }
        Ok(())
    })?;
    let missing = || {
        Error::new_spanned(
            &attribute,
            "operation requires request, response and policy",
        )
    };
    let request = request.ok_or_else(missing)?;
    let response = response.ok_or_else(missing)?;
    let derived_id = item
        .ident
        .to_string()
        .trim_start_matches("r#")
        .to_snake_case();
    let id = id.unwrap_or_else(|| syn::parse_quote!(#derived_id));
    let policy = policy.ok_or_else(missing)?;
    let name = &item.ident;
    let key = key.map(|key| {
        quote! {
            fn key(request: &Self::Request) -> Option<#sdk::operations::CacheKey> {
                (#key)(request)
            }
        }
    });
    let input_declaration = input.as_ref().map(|item| {
        quote! {
            impl #sdk::operations::InputDeclaration for #name { type Item = #item; }
        }
    });
    let output_declaration = output.as_ref().map(|item| {
        quote! {
            impl #sdk::operations::OutputDeclaration for #name { type Item = #item; }
        }
    });
    let input_type = input.as_ref().map_or_else(
        || quote!(None),
        |_| quote!(Some(::core::any::TypeId::of::<<Self as #sdk::operations::InputDeclaration>::Item>())),
    );
    let output_type = output.as_ref().map_or_else(
        || quote!(None),
        |_| quote!(Some(::core::any::TypeId::of::<<Self as #sdk::operations::OutputDeclaration>::Item>())),
    );
    if grpc.is_some() && version.is_none() {
        return Err(Error::new_spanned(
            &attribute,
            "grpc requires an explicit wire version",
        ));
    }
    let shape = match (input.is_some(), output.is_some()) {
        (false, false) => syn::Ident::new("unary", proc_macro2::Span::call_site()),
        (true, false) => syn::Ident::new("upload", proc_macro2::Span::call_site()),
        (false, true) => syn::Ident::new("download", proc_macro2::Span::call_site()),
        (true, true) => syn::Ident::new("exchange", proc_macro2::Span::call_site()),
    };
    let grpc = grpc.map(|binding| {
        quote! {
            const GRPC: Option<#sdk::operations::Grpc> = Some(#binding);
            #sdk::__private::with_protobuf! {
                fn visit_grpc<V: #sdk::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
                    Some(visitor.#shape::<Self>())
                }
            }
        }
    });
    let projection = version.as_ref().map(|_| {
        quote! {
            fn visit_wire<V: #sdk::operations::WireVisitor>(visitor: V) -> Option<V::Entry> {
                Some(visitor.#shape::<Self>())
            }
        }
    });
    let wire = version.map(|version| {
        let input_schema = input.as_ref().map(|_| quote! {
            const INPUT: Option<#sdk::schema::declaration::Definition> = Some(
                <<Self as #sdk::operations::InputDeclaration>::Item as #sdk::schema::Described>::DEFINITION
            );
        });
        let output_schema = output.as_ref().map(|_| quote! {
            const OUTPUT: Option<#sdk::schema::declaration::Definition> = Some(
                <<Self as #sdk::operations::OutputDeclaration>::Item as #sdk::schema::Described>::DEFINITION
            );
        });
        quote! {
            impl #sdk::operations::WireOperation for #name {
                const VERSION: u32 = #version;
                #grpc
                #input_schema
                #output_schema
            }
        }
    });
    let projected = quote! {
        #item
        impl #sdk::operations::OperationDeclaration for #name {
            type Request = #request;
            type Response = #response;
            const ID: &'static str = #id;
            const POLICY: #sdk::operations::OperationPolicy = #policy;
            fn endpoint_types() -> (Option<::core::any::TypeId>, Option<::core::any::TypeId>) {
                (#input_type, #output_type)
            }
            #(#constants)*
            #projection
            #key
        }
        #input_declaration
        #output_declaration
        #wire
    };
    Ok(projected)
}

/* === Private Functions === */
fn grpc_binding(
    meta: syn::meta::ParseNestedMeta<'_>,
    sdk: &HarmosPath,
) -> Result<TokenStream, Error> {
    let mut values = std::collections::BTreeMap::new();
    meta.parse_nested_meta(|field| {
        let name = field
            .path
            .get_ident()
            .map(ToString::to_string)
            .ok_or_else(|| field.error("expected package, service or method"))?;
        if !matches!(name.as_str(), "package" | "service" | "method") {
            return Err(field.error("expected package, service or method"));
        }
        let value: syn::LitStr = field.value()?.parse()?;
        if values.insert(name, value).is_some() {
            return Err(field.error("gRPC binding fields cannot repeat"));
        }
        Ok(())
    })?;
    let mut take = |name| {
        values
            .remove(name)
            .ok_or_else(|| meta.error("grpc requires package, service and method"))
    };
    let package = take("package")?;
    let service = take("service")?;
    let method = take("method")?;
    for (name, value) in [
        ("package", &package),
        ("service", &service),
        ("method", &method),
    ] {
        let text = value.value();
        let valid = |value: &str| {
            let mut bytes = value.bytes();
            bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        };
        if text.len() > 255
            || (name == "package" && text.starts_with("grpc."))
            || !(if name == "package" {
                text.split('.').all(valid)
            } else {
                valid(&text)
            })
        {
            return Err(Error::new_spanned(
                value,
                "invalid or reserved gRPC binding name",
            ));
        }
    }
    Ok(quote!(#sdk::operations::Grpc::new(#package, #service, #method)))
}

/* === Tests === */
