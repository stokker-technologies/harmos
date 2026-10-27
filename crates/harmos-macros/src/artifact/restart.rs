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
use quote::ToTokens;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, LitInt, Meta, Token};
// Local Imports
use super::set_once;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub(super) struct Policy {
    pub(super) credit: u32,
    pub(super) sustained_health_ms: u64,
    pub(super) backoff_base_ms: u64,
    pub(super) backoff_cap_ms: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Policy {
    pub(super) const NEVER: Self = Self {
        credit: 0,
        sustained_health_ms: 30_000,
        backoff_base_ms: 250,
        backoff_cap_ms: 10_000,
    };
}

/* === Public Functions === */

pub(super) fn parse(listed: &syn::MetaList) -> Result<Policy, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(listed.tokens.clone())?;
    let mut credit = None;
    let mut sustained_health_ms = None;
    let mut backoff_base_ms = None;
    let mut backoff_cap_ms = None;
    for argument in &arguments {
        let Meta::NameValue(named) = argument else {
            return Err(Error::new(argument.span(), "restart policy fields are named integer values"));
        };
        let value = syn::parse2::<LitInt>(named.value.to_token_stream())?;
        if named.path.is_ident("credit") {
            set_once(&mut credit, value, named.span(), "credit")?;
        } else if named.path.is_ident("sustained_health_ms") {
            set_once(&mut sustained_health_ms, value, named.span(), "sustained_health_ms")?;
        } else if named.path.is_ident("backoff_base_ms") {
            set_once(&mut backoff_base_ms, value, named.span(), "backoff_base_ms")?;
        } else if named.path.is_ident("backoff_cap_ms") {
            set_once(&mut backoff_cap_ms, value, named.span(), "backoff_cap_ms")?;
        } else {
            return Err(Error::new(
                named.span(),
                "restart policy fields are `credit`, `sustained_health_ms`, `backoff_base_ms`, and `backoff_cap_ms`",
            ));
        }
    }
    let required =
        |value: Option<LitInt>, name: &str| value.ok_or_else(|| Error::new(listed.span(), format!("restart policy declares `{name} = …`")));
    let credit = required(credit, "credit")?.base10_parse::<u32>()?;
    let sustained_health_ms = required(sustained_health_ms, "sustained_health_ms")?.base10_parse::<u64>()?;
    let backoff_base_ms = required(backoff_base_ms, "backoff_base_ms")?.base10_parse::<u64>()?;
    let backoff_cap_ms = required(backoff_cap_ms, "backoff_cap_ms")?.base10_parse::<u64>()?;
    if sustained_health_ms == 0 || backoff_base_ms == 0 || backoff_cap_ms < backoff_base_ms {
        return Err(Error::new(listed.span(), "restart timing is non-zero and `backoff_cap_ms` is at least `backoff_base_ms`"));
    }
    Ok(Policy {
        credit,
        sustained_health_ms,
        backoff_base_ms,
        backoff_cap_ms,
    })
}

/* === Private Functions === */

/* === Tests === */
