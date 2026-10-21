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
use syn::{Error, Item, LitInt, LitStr, Meta, Token, Type, parse::Parser, punctuated::Punctuated, spanned::Spanned};
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

pub(super) fn samples(list: &syn::MetaList) -> Result<LitInt, Error> {
    let mut samples = None;
    let mut gaps = None;
    for meta in Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())? {
        match meta {
            Meta::NameValue(pair) if pair.path.is_ident("samples") => {
                let value: LitInt = syn::parse2(pair.value.to_token_stream())?;
                if samples.replace(value).is_some() {
                    return Err(Error::new(pair.span(), "batch samples declared twice"));
                }
            }
            Meta::NameValue(pair) if pair.path.is_ident("gaps") => {
                let value: LitStr = syn::parse2(pair.value.to_token_stream())?;
                if value.value() != "report" {
                    return Err(Error::new(value.span(), "batch gaps supports only \"report\""));
                }
                if gaps.replace(value).is_some() {
                    return Err(Error::new(pair.span(), "batch gaps declared twice"));
                }
            }
            other => {
                return Err(Error::new(other.span(), "batch accepts samples = N and gaps = \"report\""));
            }
        }
    }
    let samples = samples.ok_or_else(|| Error::new(list.span(), "batch requires samples = N"))?;
    let value = samples.base10_parse::<u64>()?;
    if value == 0 || value > u32::MAX as u64 || !matches!(samples.suffix(), "" | "u64" | "u32") {
        return Err(Error::new(samples.span(), "batch samples must be an integer in 1..=4294967295"));
    }
    Ok(LitInt::new(&value.to_string(), samples.span()))
}

pub(super) fn expand(item: &mut Item, samples: Option<&LitInt>, harmos: &HarmosPath) -> Result<TokenStream, Error> {
    let Item::Struct(row) = item else {
        return if samples.is_some() {
            Err(Error::new(item.span(), "batch requires a struct with named scalar fields"))
        } else {
            Ok(quote!())
        };
    };
    let mut fields = Vec::new();
    for field in &mut row.fields {
        let mut marked = false;
        let mut retained = Vec::new();
        for attribute in std::mem::take(&mut field.attrs) {
            if attribute.path().is_ident("harmos") {
                let meta: syn::Ident = attribute.parse_args()?;
                if meta != "measure" || marked {
                    return Err(Error::new(attribute.span(), "a field accepts one #[harmos(measure)]"));
                }
                marked = true;
            } else {
                retained.push(attribute);
            }
        }
        field.attrs = retained;
        if marked {
            if samples.is_none() {
                return Err(Error::new(field.span(), "measure requires stream batch(samples = N)"));
            }
            let Type::Path(path) = &field.ty else {
                return Err(Error::new(field.span(), "measure requires a numeric scalar"));
            };
            let Some(segment) = path.path.segments.last() else { unreachable!() };
            if !matches!(segment.ident.to_string().as_str(), "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64")
                || !matches!(segment.arguments, syn::PathArguments::None)
            {
                return Err(Error::new(
                    field.span(),
                    "measure supports numeric scalar integers through 64 bits and f32/f64; arrays, buffers and optional values are not samples",
                ));
            }
            fields.push(field.ident.clone().ok_or_else(|| Error::new(field.span(), "measure requires named fields"))?);
        }
    }
    let Some(samples) = samples else {
        return Ok(quote!());
    };
    if fields.is_empty() {
        return Err(Error::new(row.span(), "batch requires at least one #[harmos(measure)] numeric scalar field"));
    }
    let names: Vec<_> = fields.iter().map(|field| field.to_string().trim_start_matches("r#").to_owned()).collect();
    let name = &row.ident;
    Ok(quote! {
        impl #harmos::streams::Batched for #name {
            const SAMPLES: u64 = #samples;
            const MEASURES: &'static [&'static str] = &[#(#names),*];
            fn measure(&self, visit: &mut dyn FnMut(f64)) { #(visit(self.#fields as f64);)* }
        }
    })
}

/* === Private Functions === */

/* === Tests === */
