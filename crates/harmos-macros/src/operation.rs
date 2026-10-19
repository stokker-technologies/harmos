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
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Error, FnArg, GenericArgument, ImplItem, ImplItemFn, ItemImpl, Meta, PathArguments, ReturnType, Token, Type};
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
    expand(attribute, item).unwrap_or_else(Error::into_compile_error)
}

/* === Private Functions === */

fn arguments(input: syn::parse::ParseStream<'_>) -> Result<Punctuated<Meta, Token![,]>, Error> {
    input.parse_terminated(argument, Token![,])
}

fn argument(input: syn::parse::ParseStream<'_>) -> Result<Meta, Error> {
    let fork = input.fork();
    let path: syn::Path = fork.parse()?;
    if ["request", "response", "input", "output", "declaration"].iter().any(|name| path.is_ident(name)) {
        let path = input.parse()?;
        let eq_token = input.parse()?;
        let value: Type = input.parse()?;
        return Ok(Meta::NameValue(syn::MetaNameValue { path, eq_token, value: syn::Expr::Verbatim(quote!(#value)) }));
    }
    input.parse()
}

fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    if let Ok(declaration) = syn::parse2::<syn::ItemStruct>(item.clone()) {
        let arguments = arguments.parse2(attribute)?;
        let mut sdk = None;
        let mut metadata = Vec::new();
        for argument in arguments {
            if let Some(value) = path::declared(&argument) {
                if sdk.is_some() {
                    return Err(Error::new_spanned(argument, "operation arguments cannot repeat"));
                }
                sdk = Some(value?);
            } else {
                metadata.push(argument);
            }
        }
        let sdk = HarmosPath::resolve(sdk);
        return crate::catalog::operation::structure(quote!(#(#metadata),*), declaration, &sdk);
    }
    let implementation: ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some() || implementation.unsafety.is_some() {
        return Err(Error::new_spanned(&implementation, "operation requires a safe inherent impl"));
    }
    let mut sdk = None;
    for argument in arguments.parse2(attribute)? {
        match path::declared(&argument) {
            Some(value) if sdk.is_none() => sdk = Some(value?),
            Some(_) => {
                return Err(Error::new_spanned(argument, "operation arguments cannot repeat"));
            }
            None => {
                return Err(Error::new_spanned(argument, "a handler declares nothing: its request type is the operation"));
            }
        }
    }
    let sdk = HarmosPath::resolve(sdk);
    let owner = &implementation.self_ty;
    let constructor = method(&implementation, "initialize")?;
    let run = method(&implementation, "run")?;
    if constructor.sig.asyncness.is_none()
        || constructor.sig.inputs.len() != 2
        || matches!(constructor.sig.inputs.first(), Some(FnArg::Receiver(_)))
    {
        return Err(Error::new_spanned(
            &constructor.sig,
            "async initialize requires an Arc<Resources> argument and the context, without self",
        ));
    }
    let resources = typed(&constructor.sig.inputs[0])?;
    let resources = parameter(resources, "Arc")?;
    if run.sig.asyncness.is_none() || !(3..=5).contains(&run.sig.inputs.len()) {
        return Err(Error::new_spanned(&run.sig, "async run requires &mut self, context, the borrowed request, then each stream endpoint"));
    }
    mutable_receiver(run)?;
    let mut context = typed(&run.sig.inputs[1])?.clone();
    context_lifetimes(&mut context);
    let declaration = borrowed(typed(&run.sig.inputs[2])?)?.clone();
    let (_, error) = result(&run.sig.output)?;
    let mut endpoints = Vec::new();
    for argument in run.sig.inputs.iter().skip(3) {
        endpoints.push(borrowed(typed(argument)?)?.clone());
    }
    let (endpoint_type, endpoint_arguments) = match endpoints.as_slice() {
        [] => (quote!(()), quote!()),
        [single] => (quote!(#single), quote!(, endpoints)),
        [left, right] => (quote!((#left, #right)), quote!(, &mut endpoints.0, &mut endpoints.1)),
        _ => unreachable!(),
    };
    let construct = match &constructor.sig.output {
        ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Path(path) if path.path.is_ident("Self")) => {
            quote!(::core::result::Result::Ok(<#owner>::initialize(resources, context).await))
        }
        ReturnType::Type(_, _) => quote!(<#owner>::initialize(resources, context).await),
        _ => {
            return Err(Error::new_spanned(&constructor.sig, "initialize must return Self or Result<Self, Error>"));
        }
    };
    let mut lifecycle = Vec::new();
    for item in &implementation.items {
        let ImplItem::Fn(method) = item else { continue };
        let name = &method.sig.ident;
        let gates = configuration(&method.attrs)?;
        match name.to_string().as_str() {
            "stop" | "cleanup" => lifecycle.push(quote! {
                #(#gates)*
                async fn #name(&mut self) -> ::core::result::Result<(), Self::Error> {
                    <#owner>::#name(self).await
                }
            }),
            "transferred" => lifecycle.push(quote! {
                #(#gates)*
                fn transferred(&self) -> bool { <#owner>::transferred(self) }
            }),
            "classify" => lifecycle.push(quote! {
                #(#gates)*
                fn classify(error: &Self::Error) -> #sdk::operations::FailureKind {
                    <#owner>::classify(error)
                }
            }),
            _ => {}
        }
    }
    let (generics, _, where_clause) = implementation.generics.split_for_impl();
    let gates = configuration(&implementation.attrs)?;
    Ok(quote! {
        #implementation
        #(#gates)*
        impl #generics #sdk::operations::Declares for #owner #where_clause {
            type Declaration = #declaration;
        }
        #(#gates)*
        impl #generics #sdk::operations::Operation for #owner #where_clause {
            type Declaration = #declaration;
            type Resources = #resources;
            type Error = #error;
            type Context<'ctx> = #context;
            type Endpoints = #endpoint_type;
            async fn initialize<'ctx>(
                resources: ::std::sync::Arc<Self::Resources>,
                context: Self::Context<'ctx>,
            ) -> ::core::result::Result<Self, Self::Error> {
                #construct
            }
            async fn run<'ctx>(
                &mut self,
                context: Self::Context<'ctx>,
                request: &mut Self::Declaration,
                endpoints: &mut Self::Endpoints,
            ) -> ::core::result::Result<#sdk::operations::Response<<Self::Declaration as #sdk::operations::OperationDeclaration>::Response>, Self::Error> {
                <#owner>::run(self, context, request #endpoint_arguments).await.map(::core::convert::Into::into)
            }
            #(#lifecycle)*
        }
    })
}

fn configuration(attributes: &[Attribute]) -> Result<Vec<Attribute>, Error> {
    fn select(meta: &Meta) -> Result<Option<Meta>, Error> {
        if meta.path().is_ident("cfg") {
            return Ok(Some(meta.clone()));
        }
        let Meta::List(list) = meta else {
            return Ok(None);
        };
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

fn method<'a>(implementation: &'a ItemImpl, name: &str) -> Result<&'a ImplItemFn, Error> {
    optional(implementation, name).ok_or_else(|| Error::new_spanned(implementation, format!("operation requires {name}")))
}

fn optional<'a>(implementation: &'a ItemImpl, name: &str) -> Option<&'a ImplItemFn> {
    implementation.items.iter().find_map(|item| match item {
        ImplItem::Fn(method) if method.sig.ident == name => Some(method),
        _ => None,
    })
}

fn typed(argument: &FnArg) -> Result<&Type, Error> {
    match argument {
        FnArg::Typed(argument) => Ok(&argument.ty),
        _ => Err(Error::new_spanned(argument, "expected a typed argument")),
    }
}

fn borrowed(ty: &Type) -> Result<&Type, Error> {
    match ty {
        Type::Reference(reference) => Ok(&reference.elem),
        _ => Err(Error::new_spanned(ty, "request and endpoints must be borrowed; the manager retains them through cleanup")),
    }
}

fn parameter<'a>(ty: &'a Type, name: &str) -> Result<&'a Type, Error> {
    if let Type::Path(path) = ty
        && let Some(segment) = path.path.segments.last()
        && segment.ident == name
        && let PathArguments::AngleBracketed(arguments) = &segment.arguments
        && arguments.args.len() == 1
        && let GenericArgument::Type(ty) = &arguments.args[0]
    {
        return Ok(ty);
    }
    Err(Error::new_spanned(ty, format!("expected {name}<T>")))
}

fn result(output: &ReturnType) -> Result<(&Type, &Type), Error> {
    if let ReturnType::Type(_, ty) = output
        && let Type::Path(path) = ty.as_ref()
        && let Some(segment) = path.path.segments.last()
        && segment.ident == "Result"
        && let PathArguments::AngleBracketed(arguments) = &segment.arguments
        && arguments.args.len() == 2
        && let (GenericArgument::Type(response), GenericArgument::Type(error)) = (&arguments.args[0], &arguments.args[1])
    {
        return Ok((response, error));
    }
    Err(Error::new_spanned(output, "run must return Result<Response, Error>"))
}

fn mutable_receiver(method: &ImplItemFn) -> Result<(), Error> {
    if matches!(method.sig.inputs.first(), Some(FnArg::Receiver(receiver)) if matches!(receiver.kind, syn::ReceiverKind::Reference(_, _, Some(_))))
    {
        return Ok(());
    }
    Err(Error::new_spanned(&method.sig, "run requires &mut self"))
}

fn context_lifetimes(ty: &mut Type) {
    match ty {
        Type::Reference(reference) => {
            if reference.lifetime.as_ref().is_none_or(|lifetime| lifetime.ident != "static") {
                reference.lifetime = Some(syn::parse_quote!('ctx));
            }
            context_lifetimes(&mut reference.elem);
        }
        Type::Path(path) => {
            for segment in &mut path.path.segments {
                if let PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                    for argument in &mut arguments.args {
                        match argument {
                            GenericArgument::Lifetime(lifetime) if lifetime.ident != "static" => *lifetime = syn::parse_quote!('ctx),
                            GenericArgument::Type(ty) => context_lifetimes(ty),
                            _ => {}
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/* === Tests === */
