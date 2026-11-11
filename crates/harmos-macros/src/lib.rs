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
mod configuration;
mod message;
mod path;
mod presentation;
mod request;
mod row;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro::TokenStream;
use syn::Error;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[proc_macro_derive(Request, attributes(request, harmos_path))]
pub fn request(item: TokenStream) -> TokenStream {
    request::derive(item.into()).into()
}

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

#[proc_macro_derive(Row, attributes(row, harmos_path, serde))]
pub fn row(item: TokenStream) -> TokenStream {
    row::derive(item.into()).into()
}

#[proc_macro_derive(Message, attributes(harmos_path, row))]
pub fn message(item: TokenStream) -> TokenStream {
    message::derive(item.into()).into()
}

/* === Private Functions === */

fn set_once<T>(place: &mut Option<T>, value: T, span: proc_macro2::Span, name: &str) -> Result<(), Error> {
    if place.replace(value).is_some() {
        return Err(Error::new(span, format!("`{name}` is declared only once")));
    }

    Ok(())
}

/* === Tests === */
