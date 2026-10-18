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
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Error, FnArg, ImplItem, ImplItemFn, ItemImpl, ReturnType, Type};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(crate) fn block(item: TokenStream, verb: &str) -> Result<ItemImpl, Error> {
    let block: ItemImpl = syn::parse2(item)?;
    if let Some((path, _)) = &block.trait_ {
        return Err(Error::new(
            path.span(),
            format!(
                "`{verb}` sits over the type's own inherent impl; a trait implementation is \
                 already the thing this attribute would generate"
            ),
        ));
    }

    Ok(block)
}

pub(crate) fn declared<'a>(block: &'a ItemImpl, verb: &str) -> Option<&'a ImplItemFn> {
    block.items.iter().find_map(|item| match item {
        ImplItem::Fn(function) if function.sig.ident == verb => Some(function),
        _ => None,
    })
}

pub(crate) fn required<'a>(block: &'a ItemImpl, verb: &str, shape: &str) -> Result<&'a ImplItemFn, Error> {
    declared(block, verb).ok_or_else(|| Error::new(block.span(), format!("declare `{shape}`")))
}

pub(crate) fn asynchronous(function: &ImplItemFn, reason: &str) -> Result<(), Error> {
    if function.sig.asyncness.is_none() {
        return Err(Error::new(function.sig.fn_token.span, format!("`{}` is `async fn`: {reason}", function.sig.ident)));
    }

    Ok(())
}

pub(crate) fn returned(function: &ImplItemFn) -> Result<Type, Error> {
    match &function.sig.output {
        ReturnType::Type(_, returned) => Ok((**returned).clone()),
        ReturnType::Default => Err(Error::new(
            function.sig.span(),
            format!(
                "`{}` answers a `Result`; what it answers with is read off that type rather than \
                 off its spelling",
                function.sig.ident
            ),
        )),
    }
}

pub(crate) fn parameter(argument: &FnArg) -> Result<Type, Error> {
    match argument {
        FnArg::Typed(typed) => Ok((*typed.ty).clone()),
        other => Err(Error::new(other.span(), "a contribution's parameters are values and borrows, never a receiver here")),
    }
}

pub(crate) fn borrowed(declared: &Type, what: &str) -> Result<Type, Error> {
    match declared {
        Type::Reference(reference) if reference.mutability.is_none() => Ok((*reference.elem).clone()),
        other => Err(Error::new(other.span(), format!("{what} is borrowed: write `&{}`", other.to_token_stream()))),
    }
}

/* === Private Functions === */

/* === Tests === */
