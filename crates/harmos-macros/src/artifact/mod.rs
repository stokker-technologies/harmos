/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(crate) mod listener;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use syn::LitStr;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn description(argument: Option<LitStr>, attributes: &[syn::Attribute]) -> String {
    argument.map(|written| written.value()).unwrap_or_else(|| documentation(attributes))
}

pub(crate) fn documentation(attributes: &[syn::Attribute]) -> String {
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

/* === Private Functions === */

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

/* === Tests === */
