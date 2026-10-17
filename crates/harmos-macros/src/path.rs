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
use syn::{Error, Meta, Path};
// Local Imports

/* === Constants === */
pub(crate) const KEY: &str = "harmos_path";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct HarmosPath(Path);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl HarmosPath {
    pub(crate) fn catalog() -> Self {
        for package in [
            "harmos",
            "dokime",
            "harmos-guest",
            "harmos-sidecar",
            "harmos-runtime",
            "harmos-domain",
        ] {
            if let Ok(found) = proc_macro_crate::crate_name(package) {
                let name = match found {
                    proc_macro_crate::FoundCrate::Itself => package.replace('-', "_"),
                    proc_macro_crate::FoundCrate::Name(name) => name,
                };
                let name = syn::Ident::new(&name, proc_macro2::Span::call_site());

                return Self(if package == "dokime" {
                    syn::parse_quote!(::#name::__harmos)
                } else {
                    syn::parse_quote!(::#name)
                });
            }
        }

        Self(syn::parse_quote!(::harmos))
    }

    pub(crate) fn resolve(explicit: Option<Path>) -> Self {
        explicit.map(Self).unwrap_or_else(Self::catalog)
    }
}

impl ToTokens for HarmosPath {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

/* === Public Functions === */
pub(crate) fn declared(meta: &Meta) -> Option<Result<Path, Error>> {
    let Meta::NameValue(named) = meta else {
        return None;
    };
    if !named.path.is_ident(KEY) {
        return None;
    }

    Some(
        syn::parse2::<Path>(named.value.to_token_stream()).map_err(|error| {
            Error::new(
                named.value.span(),
                format!(
                    "`{KEY}` names the crate path generated code resolves harmos through: {error}"
                ),
            )
        }),
    )
}

/* === Private Functions === */

/* === Tests === */
