/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod artifact;
mod catalog;
mod identity;
mod message;
mod operation;
mod path;
mod presentation;
mod row;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro::TokenStream;
use syn::Error;
// Local Imports
use identity::Sort;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[proc_macro_derive(Described, attributes(harmos_path, serde))]
pub fn described(item: TokenStream) -> TokenStream {
    let result = (|| {
        let item: syn::Item = syn::parse(item)?;
        let attributes = match &item {
            syn::Item::Struct(item) => &item.attrs,
            syn::Item::Enum(item) => &item.attrs,
            _ => unreachable!(),
        };
        let explicit = attributes
            .iter()
            .find(|attribute| attribute.path().is_ident("harmos_path"))
            .map(|attribute| attribute.parse_args::<syn::Path>())
            .transpose()?;
        message::shape(&item, &path::HarmosPath::resolve(explicit))
    })();
    result.unwrap_or_else(|error: syn::Error| error.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn catalog(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::typed::declare(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn guest_catalog(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::typed::declare_runtime(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn domain_catalog(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::typed::declare_domain(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn runtime_catalog(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::typed::declare_runtime(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn operation(attribute: TokenStream, item: TokenStream) -> TokenStream {
    operation::declare(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn guest_operation(attribute: TokenStream, item: TokenStream) -> TokenStream {
    operation::declare_runtime(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn domain_operation(attribute: TokenStream, item: TokenStream) -> TokenStream {
    operation::declare_domain(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn runtime_operation(attribute: TokenStream, item: TokenStream) -> TokenStream {
    operation::declare_runtime(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn transaction(attribute: TokenStream, item: TokenStream) -> TokenStream {
    identity::stamp(attribute.into(), item.into()).into()
}

#[proc_macro_derive(Row, attributes(row, harmos_path, serde))]
pub fn row(item: TokenStream) -> TokenStream {
    row::derive(item.into()).into()
}

#[proc_macro_attribute]
pub fn transactions(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::close(attribute.into(), item.into(), Sort::Transaction).into()
}

#[proc_macro_attribute]
pub fn records(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::close(attribute.into(), item.into(), Sort::Record).into()
}

#[proc_macro_attribute]
pub fn message(attribute: TokenStream, item: TokenStream) -> TokenStream {
    message::declare(attribute.into(), item.into()).into()
}

/* === Private Functions === */

fn set_once<T>(place: &mut Option<T>, value: T, span: proc_macro2::Span, name: &str) -> Result<(), Error> {
    if place.replace(value).is_some() {
        return Err(Error::new(span, format!("`{name}` is declared only once")));
    }

    Ok(())
}

/* === Tests === */
