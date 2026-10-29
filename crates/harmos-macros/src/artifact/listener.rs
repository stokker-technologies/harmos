/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod declaration;

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn declare(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match declaration::expand(attribute, item.clone()) {
        Ok(expanded) => expanded,
        Err(refused) => {
            let refusal = refused.to_compile_error();
            quote! { #item #refusal }
        }
    }
}
