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
use syn::spanned::Spanned;
use syn::{Error, Ident, ItemEnum, Meta, Path, Token, Type, punctuated::Punctuated};
// Local Imports
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

pub(crate) fn close(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, item.clone()) {
        Ok(closed) => closed,
        Err(refused) => {
            let refusal = refused.to_compile_error();

            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */

fn expand(attribute: TokenStream, item: TokenStream) -> ::core::result::Result<TokenStream, Error> {
    let closing = closing(attribute)?;
    let catalog: ItemEnum = syn::parse2(item)?;
    let members = members(&catalog)?;
    let name = catalog.ident.clone();
    let visibility = catalog.vis.clone();
    let state = closing.state.as_ref().ok_or_else(|| Error::new(catalog.span(), "a transaction catalog names its state"))?;
    let conversions = members.iter().map(|member| {
        let variant = &member.name;
        let definition = &member.definition;
        quote! {
            impl ::core::convert::From<#definition> for #name {
                fn from(value: #definition) -> Self { Self::#variant(value) }
            }
        }
    });
    let harmos = &closing.harmos;
    let registration = registration(&name, &visibility, &members, state, harmos);
    let crossing = crossing(&name, &visibility, &members, harmos);
    let behavior = behavior(&name, &visibility, &members, state, closing.error.as_ref(), harmos);
    Ok(quote! { #catalog #(#conversions)* #registration #crossing #behavior })
}

fn crossing(name: &Ident, visibility: &syn::Visibility, members: &[Member], harmos: &HarmosPath) -> TokenStream {
    let identities = members.iter().map(|member| {
        let variant = &member.name;
        let definition = &member.definition;
        quote!(Self::#variant(_) => <#definition as #harmos::Identified>::definition(),)
    });
    let encodings = members.iter().map(|member| {
        let variant = &member.name;
        quote!(Self::#variant(value) => ::serde::Serialize::serialize(value, serializer),)
    });
    let restorations = members.iter().map(|member| {
        let definition = &member.definition;
        quote! {
            if id.as_str() == <#definition>::ID {
                return Ok(<#definition>::restore(version, payload)?.map(::core::convert::Into::into));
            }
        }
    });
    quote! {
        impl #harmos::state::CatalogCodec for #name {
            fn identify(&self) -> (#harmos::DefinitionId, u32) { self.identify() }
            fn encode<S: ::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> { self.encode_with(serializer) }
        }
        impl #name {
            #visibility fn identify(&self) -> (#harmos::DefinitionId, u32) { match self { #(#identities)* } }
            #visibility fn encode_with<S: ::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                match self { #(#encodings)* }
            }
            #visibility fn restore<'de, D>(id: &#harmos::DefinitionId, version: u32, payload: D) -> ::core::result::Result<Option<Self>, D::Error>
            where D: ::serde::Deserializer<'de> + Copy {
                #(#restorations)*
                Ok(None)
            }
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
    let inverses = inverses(name, members, state, harmos);

    quote! {
        #declaration

        impl #harmos::state::TransactionCatalog<#state> for #name {
            fn replay(&self, prepared: &#harmos::state::Prepared, state: &mut #state) -> ::core::result::Result<(), #harmos::state::DecodeError> {
                let refusal = || #harmos::state::DecodeError::Malformed { id: prepared.id.clone(), version: prepared.version };
                let value = Self::restore(&prepared.id, prepared.version, &prepared.payload)
                    .map_err(|_| refusal())?.ok_or_else(refusal)?;
                #harmos::state::Transaction::apply(&value, state);
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

fn inverses(name: &Ident, members: &[Member], state: &Path, harmos: &HarmosPath) -> TokenStream {
    let arms = members.iter().map(|member| {
        let variant = &member.name;

        quote! {
            Self::#variant(definition) => {
                definition.__harmos_check_inverse::<#state, Self>();
                Probe(definition, before).inverse().map(#harmos::__private::Catalogued::<Self>::catalogued)
            },
        }
    });

    quote! {
        impl ::core::convert::From<#harmos::state::Never> for #name {
            fn from(never: #harmos::state::Never) -> Self { match never {} }
        }
        impl #name {
            fn __harmos_inverses(
                change: &Self,
                before: &#state,
            ) -> ::core::option::Option<Self> {
                // Method Resolution Selects The Borrowed Fallback Only Without Invertible.
                struct Probe<'a, T, S>(&'a T, &'a S);
                trait Detect {
                    type Inverse;
                    fn inverse(self) -> ::core::option::Option<Self::Inverse>;
                }
                impl<T, S> Detect for &Probe<'_, T, S> {
                    type Inverse = #harmos::state::Never;
                    fn inverse(self) -> ::core::option::Option<Self::Inverse> { None }
                }
                impl<T: #harmos::state::Invertible<S>, S> Detect for Probe<'_, T, S> {
                    type Inverse = T::Inverse;
                    fn inverse(self) -> ::core::option::Option<Self::Inverse> {
                        Some(#harmos::state::Invertible::inverse(self.0, self.1))
                    }
                }
                match change { #(#arms)* }
            }
        }
    }
}

fn registration(name: &Ident, visibility: &syn::Visibility, members: &[Member], state: &Path, harmos: &HarmosPath) -> TokenStream {
    let entries = members.iter().map(|member| {
        let definition = &member.definition;
        let variant = &member.name;
        let project = if members.len() == 1 {
            quote!({ let #name::#variant(value) = value; Some(value) })
        } else {
            quote!(match value { #name::#variant(value) => Some(value), _ => None })
        };
        quote!(.transaction(<#definition>::__harmos_entry::<#state>(), #name::#variant, |value| #project))
    });
    quote! {
        impl #name {
            #visibility fn catalog() -> #harmos::state::StateCatalog<#state, #name> {
                #harmos::state::StateCatalog::new() #(#entries)*
            }
        }
    }
}

fn closing(attribute: TokenStream) -> ::core::result::Result<Closing, Error> {
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

    Ok(Closing {
        state,
        error,
        harmos: HarmosPath::resolve(harmos_path),
    })
}

fn members(catalog: &ItemEnum) -> ::core::result::Result<Vec<Member>, Error> {
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
        members.push(Member {
            name: variant.ident.clone(),
            definition: field.ty.clone(),
        });
    }

    if members.is_empty() {
        return Err(Error::new(catalog.span(), "a catalog declares at least one definition"));
    }

    Ok(members)
}

/* === Tests === */
