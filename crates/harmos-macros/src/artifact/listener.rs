/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod declaration;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use proc_macro2::TokenStream;
use quote::quote;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(crate) fn declare(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match declaration::expand(attribute, item.clone()) {
        Ok(expanded) => expanded,
        Err(refused) => {
            let refusal = refused.to_compile_error();
            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */

/* === Tests === */
