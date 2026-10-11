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

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use proc_macro::TokenStream;
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
#[proc_macro_attribute]
pub fn catalog(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::typed::declare(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn operation(attribute: TokenStream, item: TokenStream) -> TokenStream {
    operation::declare(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn lanes(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::lanes::close(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn transaction(attribute: TokenStream, item: TokenStream) -> TokenStream {
    identity::stamp(attribute.into(), item.into(), Sort::Transaction).into()
}

#[proc_macro_attribute]
pub fn record(attribute: TokenStream, item: TokenStream) -> TokenStream {
    identity::stamp(attribute.into(), item.into(), Sort::Record).into()
}

#[proc_macro_attribute]
pub fn stream(attribute: TokenStream, item: TokenStream) -> TokenStream {
    identity::stream::stamp(attribute.into(), item.into()).into()
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
pub fn streams(attribute: TokenStream, item: TokenStream) -> TokenStream {
    catalog::streams::close(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn plugin(attribute: TokenStream, item: TokenStream) -> TokenStream {
    artifact::plugin::declare(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn observer(attribute: TokenStream, item: TokenStream) -> TokenStream {
    artifact::observer::declare(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn listener(attribute: TokenStream, item: TokenStream) -> TokenStream {
    artifact::listener::declare(attribute.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn message(attribute: TokenStream, item: TokenStream) -> TokenStream {
    message::declare(attribute.into(), item.into()).into()
}

/* === Private Functions === */

/* === Tests === */
