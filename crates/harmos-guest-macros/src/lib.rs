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
mod path;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro::TokenStream;
use syn::{Error, LitStr};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[proc_macro_attribute]
pub fn plugin(attribute: TokenStream, item: TokenStream) -> TokenStream {
    artifact::declare(attribute.into(), item.into()).into()
}

/* === Private Functions === */

fn set_once<T>(place: &mut Option<T>, value: T, span: proc_macro2::Span, name: &str) -> Result<(), Error> {
    if place.replace(value).is_some() {
        return Err(Error::new(span, format!("`{name}` is declared only once")));
    }
    Ok(())
}

fn symbol(value: &str) -> String {
    value.chars().map(|character| if character.is_ascii_alphanumeric() { character.to_ascii_uppercase() } else { '_' }).collect()
}

fn description(argument: Option<LitStr>, attributes: &[syn::Attribute]) -> String {
    argument.map(|written| written.value()).unwrap_or_else(|| documentation(attributes))
}

fn documentation(attributes: &[syn::Attribute]) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut paragraph = String::new();

    for line in attributes.iter().filter_map(documented) {
        let line = line.trim();

        if line.is_empty() {
            if !paragraph.is_empty() {
                paragraphs.push(std::mem::take(&mut paragraph));
            }

            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line);
    }

    if !paragraph.is_empty() {
        paragraphs.push(paragraph);
    }

    paragraphs.join("\n")
}

fn documented(attribute: &syn::Attribute) -> Option<String> {
    let syn::Meta::NameValue(named) = &attribute.meta else {
        return None;
    };
    if !named.path.is_ident("doc") {
        return None;
    }
    let syn::Expr::Lit(literal) = &named.value else {
        return None;
    };
    let syn::Lit::Str(text) = &literal.lit else {
        return None;
    };

    Some(text.value())
}

fn version_argument(span: proc_macro2::Span) -> Error {
    Error::new(span, "remove `version`: executable versions come from Cargo package.version")
}

/* === Tests === */
