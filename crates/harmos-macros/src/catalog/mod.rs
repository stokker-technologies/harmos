/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(crate) mod lanes;
pub(crate) mod operation;
pub(crate) mod streams;
pub(crate) mod typed;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::{Error, Ident, ItemEnum, Meta, Path, Token, Type, punctuated::Punctuated};
// Local Imports
use crate::identity::Sort;
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Closing {
    state: Option<Path>,
    error: Option<Path>,
    harmos: HarmosPath,
}

struct Member {
    name: Ident,
    definition: Type,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn close(attribute: TokenStream, item: TokenStream, sort: Sort) -> TokenStream {
    match expand(attribute, item.clone(), sort) {
        Ok(closed) => closed,
        Err(refused) => {
            let refusal = refused.to_compile_error();

            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */

fn expand(attribute: TokenStream, item: TokenStream, sort: Sort) -> Result<TokenStream, Error> {
    let closing = closing(attribute)?;
    let catalog: ItemEnum = syn::parse2(item)?;
    let members = members(&catalog)?;
    let name = catalog.ident.clone();
    let visibility = catalog.vis.clone();

    let conversions = members.iter().map(|member| {
        let variant = &member.name;
        let definition = &member.definition;

        quote! {
            #[automatically_derived]
            impl ::core::convert::From<#definition> for #name {
                fn from(definition: #definition) -> Self {
                    Self::#variant(definition)
                }
            }
        }
    });
    let state = match sort {
        Sort::Record => None,
        Sort::Transaction => {
            let Some(state) = &closing.state else {
                return Err(Error::new(
                    catalog.span(),
                    "a change catalog names the state it changes: `#[harmos::transactions(state = …)]`",
                ));
            };
            Some(state)
        }
    };
    let harmos = &closing.harmos;
    let registration = registration(&name, &visibility, &members, state, harmos);
    let crossing = crossing(&name, &visibility, &members, state, harmos);
    let projections = if state.is_none() {
        let unmatched = (members.len() > 1).then(|| quote!(_ => ::core::option::Option::None,));
        let projections = members.iter().map(|member| {
            let variant = &member.name;
            let definition = &member.definition;

            quote! {
                #[automatically_derived]
                impl #harmos::state::RecordProjection<#definition> for #name {
                    fn project(&self) -> ::core::option::Option<&#definition> {
                        match self {
                            Self::#variant(record) => ::core::option::Option::Some(record),
                            #unmatched
                        }
                    }
                }
            }
        });
        quote!(#(#projections)*)
    } else {
        TokenStream::new()
    };
    let behavior = state.map(|state| behavior(&name, &visibility, &members, state, closing.error.as_ref(), harmos)).unwrap_or_default();

    Ok(quote! {
        #[#harmos::catalog(harmos_path = #harmos)]
        #catalog
        #(#conversions)*
        #registration
        #crossing
        #projections
        #behavior
    })
}

fn crossing(name: &Ident, visibility: &syn::Visibility, members: &[Member], state: Option<&Path>, harmos: &HarmosPath) -> TokenStream {
    let identities = members.iter().map(|member| {
        let variant = &member.name;
        let definition = &member.definition;

        quote!(Self::#variant(_) => <#definition>::DEFINITION,)
    });
    let encodings = members.iter().map(|member| {
        let variant = &member.name;

        quote!(Self::#variant(definition) => ::serde::Serialize::serialize(definition, serializer),)
    });
    let named = members.iter().map(|member| &member.definition);
    let restore = match state {
        Some(state) => quote! {



            #[allow(clippy::type_complexity)]
            #visibility fn restore<'de, D>(
                id: &#harmos::DefinitionId,
                version: u32,
                payload: D,
            ) -> ::core::result::Result<
                ::core::option::Option<(
                    Self,
                    ::core::option::Option<
                        ::std::boxed::Box<
                            dyn ::core::ops::FnOnce(&mut #state) + ::core::marker::Send,
                        >,
                    >,
                )>,
                D::Error,
            >
            where
                D: ::serde::Deserializer<'de> + ::core::marker::Copy,
            {
                #(
                    if id.as_str() == <#named>::DEFINITION.1 {
                        return ::core::result::Result::Ok(
                            <#named>::restore::<#state, D>(version, payload)?.map(
                                |(definition, applying)| {
                                    (::core::convert::From::from(definition), applying)
                                },
                            ),
                        );
                    }
                )*

                ::core::result::Result::Ok(::core::option::Option::None)
            }
        },
        None => quote! {



            #visibility fn restore<'de, D>(
                id: &#harmos::DefinitionId,
                version: u32,
                payload: D,
            ) -> ::core::result::Result<::core::option::Option<Self>, D::Error>
            where
                D: ::serde::Deserializer<'de> + ::core::marker::Copy,
            {
                #(
                    if id.as_str() == <#named>::DEFINITION.1 {
                        return ::core::result::Result::Ok(
                            <#named>::restore::<D>(version, payload)?
                                .map(::core::convert::From::from),
                        );
                    }
                )*

                ::core::result::Result::Ok(::core::option::Option::None)
            }
        },
    };
    let record_catalog = state.is_none().then(|| {
        quote! {
            impl #harmos::state::RecordCatalog for #name {
                fn restore(id: &#harmos::DefinitionId, version: u32, payload: &#harmos::__private::serde_json::Value)
                    -> ::core::result::Result<::core::option::Option<Self>, #harmos::__private::serde_json::Error> {
                    Self::restore(id, version, payload)
                }
            }
        }
    });

    quote! {
        #record_catalog
        impl #harmos::state::CatalogCodec for #name {
            fn identify(&self) -> (#harmos::DefinitionId, u32) { self.identify() }
            fn encode<S: ::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> { self.encode_with(serializer) }
        }
        impl #name {

            #visibility fn identify(&self) -> (#harmos::DefinitionId, u32) {
                let (_, id, version) = match self { #(#identities)* };

                (#harmos::DefinitionId::new(id), version)
            }




            #visibility fn encode_with<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                match self { #(#encodings)* }
            }

            #restore
        }
    }
}

fn behavior(
    name: &Ident, visibility: &syn::Visibility, members: &[Member], state: &Path, error: Option<&Path>, harmos: &HarmosPath,
) -> TokenStream {
    let aggregated = Ident::new(&format!("{name}Error"), name.span());
    let (refusal, declaration, wrap): (TokenStream, TokenStream, Vec<TokenStream>) = match error {
        // The application aggregates its own refusals, so every variant's error
        // travels in through the conversion the application declared.
        Some(named) => (quote!(#named), quote!(), members.iter().map(|_| quote!(::core::convert::Into::into)).collect()),
        None => (
            quote!(#aggregated),
            aggregate(&aggregated, visibility, members, state, harmos),
            members
                .iter()
                .map(|member| {
                    let variant = &member.name;

                    quote!(#aggregated::#variant)
                })
                .collect(),
        ),
    };

    let checks = members.iter().zip(&wrap).map(|(member, wrap)| {
        let variant = &member.name;

        quote!(Self::#variant(definition) => #harmos::state::Transaction::check(definition, state).map_err(#wrap),)
    });
    let applies = members.iter().map(|member| {
        let variant = &member.name;

        quote!(Self::#variant(definition) => #harmos::state::Transaction::apply(definition, state),)
    });

    let records = members.iter().map(|member| {
        let variant = &member.name;
        quote!(Self::#variant(definition) => #harmos::state::Transaction::records(definition, state, out),)
    });
    let inverses = inverses(name, members, state);

    quote! {
        #declaration

        impl #harmos::state::TransactionCatalog<#state> for #name {
            fn replay(&self, prepared: &#harmos::state::Prepared, state: &mut #state) -> ::core::result::Result<(), #harmos::state::DecodeError> {
                let refusal = || #harmos::state::DecodeError::Malformed { id: prepared.id.clone(), version: prepared.version };
                let (value, applying) = Self::restore(&prepared.id, prepared.version, &prepared.payload)
                    .map_err(|_| refusal())?.ok_or_else(refusal)?;
                if let Some(apply) = applying { apply(state); } else { #harmos::state::Transaction::apply(&value, state); }
                ::core::result::Result::Ok(())
            }
        }

        #[automatically_derived]
        impl #harmos::state::Transaction<#state> for #name {
            type Error = #refusal;

            fn check(&self, state: &#state) -> ::core::result::Result<(), Self::Error> {
                match self { #(#checks)* }
            }

            fn records(&self, state: &#state, out: &mut #harmos::state::Records) {
                match self { #(#records)* }
            }

            fn apply(&self, state: &mut #state) {
                match self { #(#applies)* }
            }
        }

        #inverses
    }
}

fn aggregate(aggregated: &Ident, visibility: &syn::Visibility, members: &[Member], state: &Path, harmos: &HarmosPath) -> TokenStream {
    let variants = members.iter().map(|member| {
        let variant = &member.name;
        let definition = &member.definition;

        quote! {

            #variant(<#definition as #harmos::state::Transaction<#state>>::Error)
        }
    });
    let displays = members.iter().map(|member| {
        let variant = &member.name;

        quote!(Self::#variant(refusal) => ::core::fmt::Display::fmt(refusal, formatter),)
    });

    quote! {

        #[derive(Debug)]
        #visibility enum #aggregated { #(#variants),* }

        #[automatically_derived]
        impl ::core::fmt::Display for #aggregated {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self { #(#displays)* }
            }
        }

        #[automatically_derived]
        impl ::std::error::Error for #aggregated {}
    }
}

fn inverses(name: &Ident, members: &[Member], state: &Path) -> TokenStream {
    let arms = members.iter().map(|member| {
        let variant = &member.name;

        quote! {
            Self::#variant(definition) => definition.__harmos_inverse::<#state, Self>(before),
        }
    });

    quote! {
        impl #name {


            fn __harmos_inverses(
                change: &Self,
                before: &#state,
            ) -> ::core::option::Option<Self> {
                match change { #(#arms)* }
            }
        }
    }
}

fn registration(name: &Ident, visibility: &syn::Visibility, members: &[Member], state: Option<&Path>, harmos: &HarmosPath) -> TokenStream {
    let declared = members.iter().map(|member| {
        let definition = &member.definition;

        quote! {
            {
                let (kind, id, version) = <#definition>::DEFINITION;
                catalogued.push((kind, #harmos::DefinitionId::new(id), version));

                for (kind, id, version) in <#definition>::HISTORY {
                    catalogued.push((kind, #harmos::DefinitionId::new(id), version));
                }
            }
        }
    });
    let (output, construct) = match state {
        Some(state) => (
            quote!(#harmos::state::TransactionDefinitions<#state, #name>),
            quote! {
                fn decode(
                    id: &#harmos::DefinitionId,
                    version: u32,
                    bytes: &[u8],
                ) -> ::core::result::Result<::core::option::Option<#name>, #harmos::state::DecodeError> {
                    let malformed = || #harmos::state::DecodeError::Malformed { id: id.clone(), version };
                    let payload: #harmos::__private::serde_json::Value =
                        #harmos::__private::serde_json::from_slice(bytes).map_err(|_| malformed())?;
                    let restored = #name::restore(id, version, &payload).map_err(|_| malformed())?;
                    Ok(restored.map(|(transaction, _apply)| transaction))
                }
                #harmos::state::TransactionDefinitions {
                    definitions: catalogued,
                    decode,
                    inverses: #name::__harmos_inverses,
                }
            },
        ),
        None => (
            quote!(#harmos::state::RecordDefinitions<#name>),
            quote!(#harmos::state::RecordDefinitions {
                definitions: catalogued,
                marker: ::core::marker::PhantomData,
            }),
        ),
    };
    quote! {
        impl #name {
            #visibility fn catalog() -> #output {
                let mut catalogued = ::std::vec::Vec::new();
                #(#declared)*
                #construct
            }
        }
    }
}

fn closing(attribute: TokenStream) -> Result<Closing, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut state = None;
    let mut error = None;
    let mut harmos_path: Option<Path> = None;

    for argument in &arguments {
        if let Some(parsed) = path::declared(argument) {
            harmos_path = Some(parsed?);
            continue;
        }
        match argument {
            Meta::NameValue(named) if named.path.is_ident("state") => {
                state = Some(syn::parse2::<Path>(named.value.to_token_stream())?);
            }
            Meta::NameValue(named) if named.path.is_ident("error") => {
                error = Some(syn::parse2::<Path>(named.value.to_token_stream())?);
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "a catalog declares `state = …` and optionally `error = …` and \
                     `harmos_path = …`",
                ));
            }
        }
    }

    Ok(Closing { state, error, harmos: HarmosPath::resolve(harmos_path) })
}

fn members(catalog: &ItemEnum) -> Result<Vec<Member>, Error> {
    let mut members = Vec::new();

    for variant in &catalog.variants {
        let syn::Fields::Unnamed(fields) = &variant.fields else {
            return Err(Error::new(
                variant.span(),
                "a catalog variant wraps exactly one definition type, so the definition keeps \
                 its own file, its own behavior, and its own history",
            ));
        };
        let [field] = &fields.unnamed.iter().collect::<Vec<_>>()[..] else {
            return Err(Error::new(variant.span(), "a catalog variant wraps exactly one definition type"));
        };
        members.push(Member { name: variant.ident.clone(), definition: field.ty.clone() });
    }

    if members.is_empty() {
        return Err(Error::new(catalog.span(), "a catalog declares at least one definition"));
    }

    Ok(members)
}

/* === Tests === */
