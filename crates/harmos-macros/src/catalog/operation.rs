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
use syn::parse::Parse;
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

pub(crate) fn structure(attribute: TokenStream, mut item: ItemStruct, sdk: &HarmosPath, executable: bool) -> Result<TokenStream, Error> {
    if !item.generics.params.is_empty() {
        return Err(Error::new_spanned(&item.generics, "an operation implementation struct cannot be generic"));
    }
    let attribute: Attribute = syn::parse_quote!(#[operation(#attribute)]);
    let mut seen = BTreeSet::new();
    let mut resources = Vec::<Type>::new();
    let mut operations = Vec::<Type>::new();
    let mut manual = false;
    let mut response = None::<Type>;
    let mut input = None::<Type>;
    let mut output = None::<Type>;
    let mut id = None::<Expr>;
    let mut policy = None::<Expr>;
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
            "resources" | "operations" => {
                let content;
                let value = meta.value()?;
                syn::bracketed!(content in value);
                let types = content.parse_terminated(Type::parse, syn::Token![,])?;
                let target = if name == "resources" { &mut resources } else { &mut operations };
                target.extend(types);
            }
            "codec" => {
                let codec: syn::Ident = meta.value()?.parse()?;
                if codec != "manual" {
                    return Err(meta.error("codec accepts only manual"));
                }
                manual = true;
            }
            "response" => response = Some(meta.value()?.parse()?),
            "input" => input = Some(meta.value()?.parse()?),
            "output" => output = Some(meta.value()?.parse()?),
            "id" => id = Some(meta.value()?.parse()?),
            "policy" => policy = Some(meta.value()?.parse()?),
            "version" => return Err(meta.error("remove `version`: operation identity is a structural fingerprint")),
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
    let missing = |field| Error::new_spanned(&attribute, format!("operation requires `{field}`"));
    let response = response.unwrap_or_else(|| syn::parse_quote!(()));
    let policy = policy.ok_or_else(|| missing("policy"))?;
    let name = item.ident.clone();
    let gates = crate::operation::configuration(&item.attrs)?;
    let derived = name.to_string().trim_start_matches("r#").to_snake_case();
    let id = id.unwrap_or_else(|| syn::parse_quote!(#derived));

    let direction =
        |item: Option<Type>| item.map_or_else(|| quote!(#sdk::operations::NoStream), |item| quote!(#sdk::operations::Stream<#item>));
    let input = direction(input);
    let output = direction(output);
    let description = description.map(|value| quote!(const DESCRIPTION: &'static str = #value;));
    let visibility = visibility.map(|value| {
        let value = match &value {
            Expr::Path(path) if path.path.is_ident("Public") || path.path.is_ident("Host") || path.path.is_ident("Private") => {
                quote!(#sdk::operations::Visibility::#value)
            }
            _ => quote!(#value),
        };
        quote!(const VISIBILITY: #sdk::operations::Visibility = #value;)
    });
    let replay = replay.map(|value| quote!(const REPLAY: #sdk::operations::ReplaySafety = #value;));
    let grpc_visibility = grpc.as_ref().map(|_| {
        quote! {
            #(#gates)*
            const _: () = assert!(
                !::core::matches!(<#name as #sdk::OperationDeclaration>::VISIBILITY, #sdk::operations::Visibility::Private),
                "Private operations cannot declare a gRPC binding"
            );
        }
    });
    let grpc = grpc.map(|(package, service, method)| {
        quote! {
            const GRPC: ::core::option::Option<#sdk::operations::Grpc> =
                ::core::option::Option::Some(#sdk::operations::Grpc::new(#package, #service, #method));
        }
    });
    let grant = grant.map(|value| {
        quote! {
            fn grant(request: &Self::Request) -> ::core::option::Option<#sdk::__private::PluginGrant> { (#value)(request) }
        }
    });
    let key = key.map(|value| {
        quote! {
            fn key(request: &Self::Request) -> ::core::option::Option<#sdk::operations::CacheKey> { (#value)(request) }
        }
    });
    let clone = item.attrs.iter().filter(|attribute| attribute.path().is_ident("derive")).any(|attribute| {
        attribute
            .parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
            .is_ok_and(|paths| paths.iter().any(|path| path.is_ident("Clone")))
    });
    if !clone && !manual {
        item.attrs.push(syn::parse_quote!(#[derive(Clone)]));
    }
    let message = if manual { quote!(#item) } else { crate::message::expand(quote!(harmos_path = #sdk), quote!(#item))? };
    let execution_gate = sdk.execution_gate();
    let requirements = resources.iter().map(|resource| {
        quote! {
            #(#gates)*
            #execution_gate
            impl #sdk::operations::Requires<#resource> for #name {}
        }
    });
    let invokes = operations.iter().map(|operation| {
        quote! {
            #(#gates)*
            impl #sdk::operations::Invokes<#operation> for #name {}
        }
    });
    let execution = executable.then(|| {
        quote! {
            #(#invokes)*
            #[allow(unexpected_cfgs)]
            const _: () = {
                #(#requirements)*
                #(#gates)*
                #execution_gate
                impl #sdk::Dependent for #name {
                    const RESOURCES: &'static [#sdk::Resource] = &[#(#sdk::Resource::of::<#resources>()),*];
                }
            };
        }
    });
    Ok(quote! {
        #message
        #grpc_visibility

        #(#gates)*
        const _: () = assert!(
            #sdk::__private::operation_identity(#id),
            "operation id must contain dotted [a-z][a-z0-9_]* segments"
        );

        #(#gates)*
        impl #sdk::OperationDeclaration for #name {
            type Request = Self;
            type Response = #response;
            type Input = #input;
            type Output = #output;
            const ID: &'static str = #id;
            const OPERATIONS: &'static [#sdk::operations::WireDeclaration] = &[#(#sdk::operations::WireDeclaration::of::<#operations>()),*];
            const POLICY: #sdk::OperationPolicy = #policy;
            #description #visibility #replay #grpc #grant #key
        }

        #(#gates)*
        #[allow(unexpected_cfgs)]
        impl #name {
            pub fn definition() -> #sdk::OperationDefinition {
                <Self as #sdk::OperationDeclaration>::definition()
            }


        }
        #execution
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
