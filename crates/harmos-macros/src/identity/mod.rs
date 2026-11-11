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
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Ident, Item, LitStr, Meta, Path, Token};
// Local Imports
use crate::path::{self, HarmosPath};
use crate::set_once;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Declaration {
    id: Option<LitStr>,
    undo: Option<Path>,
    harmos: HarmosPath,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn stamp(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, &item) {
        Ok(stamped) => stamped,
        Err(refused) => {
            let refusal = refused.to_compile_error();

            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */

fn undoing(name: &Ident, inverse: &Path, harmos: &HarmosPath) -> TokenStream {
    quote! {
            #[automatically_derived]
            impl #harmos::__private::Paired for #name {
                type Inverse = #inverse;
            }

            // Inverses come in pairs, and the pair is checked where it is
            // declared rather than where it is used: the round trip through
            // both declarations lands back on this change, or it does not
            // compile.
            const _: () = {
                fn mutual<A, B: #harmos::__private::Mutual<A>>() {}

                fn pairs() {
                    mutual::<#name, <#inverse as #harmos::__private::Paired>::Inverse>();
                }
            };

            impl #name {

                // The Described Body Names the Declared Inverse in Its Header,
                // So A Body That Returns Another Change Does Not Compile.
                pub fn __harmos_check_inverse<S, C>(&self)
                where
                    Self: #harmos::state::Invertible<S, Inverse = #inverse>,
                    #inverse: #harmos::__private::Catalogued<C>,
                {
                }
            }
    }
}

fn expand(attribute: TokenStream, item: &TokenStream) -> Result<TokenStream, Error> {
    let mut declared = declaration(attribute)?;
    let parsed: Item = syn::parse2(item.clone())?;
    let name = named(&parsed)?;

    // `inverse = Self` is the change that is its own pair, and every check
    // below reads one type name, so the shorthand is resolved once here.
    if let Some(inverse) = &mut declared.undo
        && inverse.is_ident("Self")
    {
        *inverse = Path::from(name.clone());
    }
    let id = declared.id.as_ref().ok_or_else(|| Error::new(parsed.span(), "a definition requires id"))?;
    let harmos = &declared.harmos;
    let restore = restore(harmos);
    let undoing = match &declared.undo {
        Some(inverse) => undoing(name, inverse, harmos),
        None => quote! {
            impl #name {
                pub fn __harmos_check_inverse<S, C>(&self) {}
            }
        },
    };

    let transaction = quote! {
        impl<S> #harmos::state::TransactionCatalog<S> for #name
        where Self: #harmos::state::Transaction<S>, S: 'static {
            fn replay(&self, prepared: &#harmos::state::Prepared, state: &mut S) -> ::core::result::Result<(), #harmos::state::DecodeError> {
                let refusal = || #harmos::state::DecodeError::Malformed { id: prepared.id.clone(), version: prepared.version };
                if prepared.id.as_str() != Self::ID { return Err(refusal()); }
                let value = Self::restore(prepared.version, &prepared.payload)
                    .map_err(|_| refusal())?.ok_or_else(refusal)?;
                #harmos::state::Transaction::apply(&value, state);
                Ok(())
            }
        }
    };

    Ok(quote! {
        #item

        #transaction
        #undoing

        impl #name {

            pub const ID: &'static str = #id;



            #restore
        }

        #[automatically_derived]
        impl #harmos::Identified for #name {
            const ID: &'static str = #id;
            fn definition() -> (#harmos::DefinitionId, u32) {
                static VERSION: ::std::sync::OnceLock<u32> = ::std::sync::OnceLock::new();
                let version = *VERSION.get_or_init(|| {
                    let schema = #harmos::schema::graph::Schema::from_declaration(
                        <Self as #harmos::schema::Described>::DEFINITION, &#harmos::config::SchemaSettings::default()
                    ).expect("valid stored data schema");
                    #harmos::schema::identity::revision(&schema).expect("valid stored data revision")
                });
                (#harmos::DefinitionId::new(#id), version)
            }
        }
    })
}

fn restore(harmos: &HarmosPath) -> TokenStream {
    quote! {
        pub fn restore<'de, D>(version: u32, payload: D) -> ::core::result::Result<::core::option::Option<Self>, D::Error>
        where D: ::serde::Deserializer<'de> + ::core::marker::Copy {
            if version != <Self as #harmos::Identified>::definition().1 { return Ok(None); }
            Ok(Some(::serde::Deserialize::deserialize(payload)?))
        }
    }
}

fn declaration(attribute: TokenStream) -> Result<Declaration, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut id = None;
    let mut undo: Option<Path> = None;
    let mut harmos_path: Option<Path> = None;

    for argument in &arguments {
        if let Some(parsed) = path::declared(argument) {
            set_once(&mut harmos_path, parsed?, argument.span(), path::KEY)?;
            continue;
        }
        match &argument {
            Meta::NameValue(named) if named.path.is_ident("id") => {
                id = Some(syn::parse2::<LitStr>(named.value.to_token_stream())?);
            }
            Meta::NameValue(named) if named.path.is_ident("inverse") => {
                set_once(&mut undo, syn::parse2::<Path>(named.value.to_token_stream())?, named.span(), "inverse")?;
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "a transaction declares `id`, a derived schema revision, optional `inverse` and `harmos_path`",
                ));
            }
        }
    }

    Ok(Declaration {
        id,
        undo,
        harmos: HarmosPath::resolve(harmos_path),
    })
}

fn named(item: &Item) -> Result<&Ident, Error> {
    match item {
        Item::Struct(shape) => Ok(&shape.ident),
        Item::Enum(shape) => Ok(&shape.ident),
        other => Err(Error::new(other.span(), "an authoring attribute belongs on the struct or enum the definition is")),
    }
}

/* === Tests === */
