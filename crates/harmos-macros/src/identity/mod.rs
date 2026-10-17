/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod batch;
pub(crate) mod stream;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use proc_macro2::{Spacing, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Ident, Item, LitInt, LitStr, Meta, Path, Token};
// Local Imports
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Declaration {
    id: Option<LitStr>,
    version: LitInt,
    history: Vec<Path>,
    undo: Option<Undo>,
    harmos: HarmosPath,
}

/* === Enums === */
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sort {
    Transaction,
    Record,
}

enum Undo {
    Inverse(Path),
    Irreversible(Path),
}

/* === Traits === */

/* === Implementations === */
impl Sort {
    pub(crate) fn kind(self, harmos: &HarmosPath) -> TokenStream {
        match self {
            Self::Transaction => quote!(#harmos::state::Kind::Transaction),
            Self::Record => quote!(#harmos::state::Kind::Record),
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Self::Transaction => "transaction",
            Self::Record => "record",
        }
    }
}

impl Undo {
    fn span(&self) -> proc_macro2::Span {
        match self {
            Self::Inverse(inverse) => inverse.span(),
            Self::Irreversible(marker) => marker.span(),
        }
    }
}

/* === Public Functions === */
pub(crate) fn stamp(attribute: TokenStream, item: TokenStream, sort: Sort) -> TokenStream {
    match expand(attribute, &item, sort) {
        Ok(stamped) => stamped,
        Err(refused) => {
            let refusal = refused.to_compile_error();

            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */
fn undoing(name: &Ident, undo: &Undo, harmos: &HarmosPath) -> TokenStream {
    match undo {
        Undo::Inverse(inverse) => quote! {
            #[automatically_derived]
            impl #harmos::state::Undoable for #name {
                type Inverse = #inverse;
            }

            // Inverses come in pairs, and the pair is checked where it is
            // declared rather than where it is used: the round trip through
            // both declarations lands back on this change, or it does not
            // compile.
            const _: () = {
                fn mutual<A, B: #harmos::__private::Mutual<A>>() {}

                fn pairs() {
                    mutual::<#name, <#inverse as #harmos::state::Undoable>::Inverse>();
                }
            };

            impl #name {

                pub fn __harmos_inverse<S, C>(&self, before: &S) -> ::core::option::Option<C>
                where
                    Self: #harmos::state::Invert<S>,
                    #inverse: #harmos::__private::Catalogued<C>,
                {
                    ::core::option::Option::Some(#harmos::__private::Catalogued::catalogued(
                        #harmos::state::Invert::invert(self, before),
                    ))
                }
            }
        },
        Undo::Irreversible(_) => quote! {
            impl #name {

                pub fn __harmos_inverse<S, C>(&self, _before: &S) -> ::core::option::Option<C> {
                    ::core::option::Option::None
                }
            }
        },
    }
}

fn expand(attribute: TokenStream, item: &TokenStream, sort: Sort) -> Result<TokenStream, Error> {
    let mut declared = declaration(attribute, sort)?;
    let parsed: Item = syn::parse2(item.clone())?;
    let name = named(&parsed)?;
    let version = &declared.version;

    // `inverse = Self` is the change that is its own pair, and every check
    // below reads one type name, so the shorthand is resolved once here.
    if let Some(Undo::Inverse(inverse)) = &mut declared.undo
        && inverse.is_ident("Self")
    {
        *inverse = Path::from(name.clone());
    }
    if declared.id.is_none()
        && let Some(undo) = &declared.undo
    {
        return Err(Error::new(
            undo.span(),
            "a historical shape answers no undo question of its own; the family root declares \
             the inverse for every version of the change",
        ));
    }
    if declared.id.is_some() && sort == Sort::Transaction && declared.undo.is_none() {
        return Err(Error::new(
            declared.version.span(),
            "a transaction declares what undoes it: `inverse = …` names the change that takes \
             it back, `inverse = Self` says it is its own, and `irreversible` says it cannot be \
             taken back",
        ));
    }

    let Some(id) = &declared.id else {
        // A historical shape: sealed version metadata, and deliberately no
        // descriptor, so it cannot be registered as a definition of its own.
        return Ok(quote! {
            #item

            impl #name {

                pub const VERSION: u32 = #version;
            }
        });
    };

    let harmos = &declared.harmos;
    let kind = sort.kind(harmos);
    let count = declared.history.len();
    let history = declared.history.iter().map(|historical| quote!((#kind, #id, <#historical>::VERSION)));
    let restore = restore(&declared, sort);
    let undoing = declared.undo.as_ref().map(|undo| undoing(name, undo, harmos)).unwrap_or_default();

    let historical_apply = &declared.history;
    let record_marker = match sort {
        Sort::Record => quote! { impl #harmos::state::Record for #name {} },
        Sort::Transaction => quote! {
            impl<S> #harmos::state::TransactionCatalog<S> for #name
            where Self: #harmos::state::Transaction<S>, S: 'static, #(#historical_apply: #harmos::state::Transaction<S>,)* {
                fn replay(&self, prepared: &#harmos::state::Prepared, state: &mut S) -> ::core::result::Result<(), #harmos::state::DecodeError> {
                    let refusal = || #harmos::state::DecodeError::Malformed { id: prepared.id.clone(), version: prepared.version };
                    if prepared.id.as_str() != Self::DEFINITION.1 { return Err(refusal()); }
                    let (value, applying) = Self::restore::<S, _>(prepared.version, &prepared.payload)
                        .map_err(|_| refusal())?.ok_or_else(refusal)?;
                    if let Some(apply) = applying { apply(state); } else { #harmos::state::Transaction::apply(&value, state); }
                    Ok(())
                }
            }
        },
    };

    Ok(quote! {
        #item

        #record_marker
        #undoing

        impl #name {

            pub const DEFINITION: (#harmos::state::Kind, &'static str, u32) = (#kind, #id, #version);


            pub const HISTORY: [(#harmos::state::Kind, &'static str, u32); #count] = [#(#history),*];

            #restore
        }

        #[automatically_derived]
        impl #harmos::Identified for #name {
            fn definition() -> (#harmos::DefinitionId, u32) {
                (#harmos::DefinitionId::new(#id), #version)
            }
        }
    })
}

fn restore(declared: &Declaration, sort: Sort) -> TokenStream {
    let version = &declared.version;
    let historical = &declared.history;
    let harmos = &declared.harmos;

    match sort {
        Sort::Transaction => quote! {



            #[allow(clippy::type_complexity)]
            pub fn restore<'de, S, D>(
                version: u32,
                payload: D,
            ) -> ::core::result::Result<
                ::core::option::Option<(
                    Self,
                    ::core::option::Option<
                        ::std::boxed::Box<dyn ::core::ops::FnOnce(&mut S) + ::core::marker::Send>,
                    >,
                )>,
                D::Error,
            >
            where
                D: ::serde::Deserializer<'de> + ::core::marker::Copy,
                S: 'static,
                #(#historical: #harmos::state::Transaction<S>,)*
            {
                if version == #version {
                    // The stored version is the one this build writes, so the
                    // catalog's own `apply` is the implementation that wrote it.
                    let current: Self = ::serde::Deserialize::deserialize(payload)?;

                    return ::core::result::Result::Ok(::core::option::Option::Some((
                        current,
                        ::core::option::Option::None,
                    )));
                }
                #(
                    if version == <#historical>::VERSION {
                        let applying: #historical = ::serde::Deserialize::deserialize(payload)?;
                        let carried: #historical = ::serde::Deserialize::deserialize(payload)?;

                        return ::core::result::Result::Ok(::core::option::Option::Some((
                            ::core::convert::From::from(carried),
                            ::core::option::Option::Some(::std::boxed::Box::new(
                                move |state: &mut S| {
                                    #harmos::state::Transaction::apply(&applying, state)
                                },
                            )),
                        )));
                    }
                )*

                ::core::result::Result::Ok(::core::option::Option::None)
            }
        },
        Sort::Record => quote! {



            pub fn restore<'de, D>(
                version: u32,
                payload: D,
            ) -> ::core::result::Result<::core::option::Option<Self>, D::Error>
            where
                D: ::serde::Deserializer<'de> + ::core::marker::Copy,
            {
                if version == #version {
                    return ::core::result::Result::Ok(::core::option::Option::Some(
                        ::serde::Deserialize::deserialize(payload)?,
                    ));
                }
                #(
                    if version == <#historical>::VERSION {
                        let historical: #historical = ::serde::Deserialize::deserialize(payload)?;

                        return ::core::result::Result::Ok(::core::option::Option::Some(
                            ::core::convert::TryFrom::try_from(historical)
                                .map_err(::serde::de::Error::custom)?,
                        ));
                    }
                )*

                ::core::result::Result::Ok(::core::option::Option::None)
            }
        },
    }
}

fn declaration(attribute: TokenStream, sort: Sort) -> Result<Declaration, Error> {
    let span = attribute.span();
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut id = None;
    let mut version = None;
    let mut history = Vec::new();
    let mut undo: Option<Undo> = None;
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
            Meta::NameValue(named) if named.path.is_ident("version") => {
                version = Some(syn::parse2::<LitInt>(named.value.to_token_stream())?);
            }
            Meta::List(listed) if listed.path.is_ident("history") => {
                history = catalogued(listed)?;
            }
            Meta::NameValue(named) if named.path.is_ident("inverse") => {
                answered(&undo, named.span())?;
                undo = Some(Undo::Inverse(syn::parse2::<Path>(named.value.to_token_stream())?));
            }
            Meta::Path(marker) if marker.is_ident("irreversible") => {
                answered(&undo, marker.span())?;
                undo = Some(Undo::Irreversible(marker.clone()));
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    format!(
                        "a {} declares `id = \"…\"`, `version = …`, and `history(…)`{}, and may \
                         declare `harmos_path = …`",
                        sort.noun(),
                        match sort {
                            Sort::Transaction =>
                                ", and answers the undo question with \
                                                  `inverse = …` or `irreversible`",
                            Sort::Record => "",
                        }
                    ),
                ));
            }
        }
    }

    if sort == Sort::Record
        && let Some(undo) = &undo
    {
        return Err(Error::new(
            undo.span(),
            "a fact changes nothing, so nothing takes it back; only a transaction answers the \
             undo question",
        ));
    }

    let Some(version) = version else {
        return Err(Error::new(span, format!("a {} declares the version it is written with", sort.noun())));
    };
    if id.is_none() && !history.is_empty() {
        return Err(Error::new(span, "a historical shape carries no history of its own; the family root owns the catalog"));
    }

    Ok(Declaration { id, version, history, undo, harmos: HarmosPath::resolve(harmos_path) })
}

fn set_once<T>(place: &mut Option<T>, value: T, span: proc_macro2::Span, name: &str) -> Result<(), Error> {
    if place.replace(value).is_some() {
        return Err(Error::new(span, format!("`{name}` is declared only once")));
    }

    Ok(())
}

fn answered(undo: &Option<Undo>, span: proc_macro2::Span) -> Result<(), Error> {
    match undo {
        Some(_) => Err(Error::new(
            span,
            "a change answers the undo question once: either it names the inverse that takes it \
             back or it is `irreversible`",
        )),
        None => Ok(()),
    }
}

fn catalogued(listed: &syn::MetaList) -> Result<Vec<Path>, Error> {
    let tokens: Vec<TokenTree> = listed.tokens.clone().into_iter().collect();
    let chained = tokens.iter().enumerate().any(|(at, token)| match token {
        // A `::` separator is two colon tokens, the first joined to the second.
        // A colon that neither joins the token after it nor follows one that
        // did is a lone `:`, which no path spelling produces.
        TokenTree::Punct(punctuation) if punctuation.as_char() == ':' => punctuation.spacing() == Spacing::Alone && !separates(&tokens, at),
        TokenTree::Punct(punctuation) => matches!(punctuation.as_char(), '-' | '>' | '=' | '|'),
        _ => false,
    });
    if chained {
        return Err(Error::new(
            listed.span(),
            "a history catalog is flat: list every historical type against the current state, \
             not a chain of predecessors",
        ));
    }

    Ok(Punctuated::<Path, Token![,]>::parse_terminated.parse2(listed.tokens.clone())?.into_iter().collect())
}

fn separates(tokens: &[TokenTree], at: usize) -> bool {
    matches!(
        at.checked_sub(1).and_then(|before| tokens.get(before)),
        Some(TokenTree::Punct(punctuation))
            if punctuation.as_char() == ':' && punctuation.spacing() == Spacing::Joint
    )
}

fn named(item: &Item) -> Result<&Ident, Error> {
    match item {
        Item::Struct(shape) => Ok(&shape.ident),
        Item::Enum(shape) => Ok(&shape.ident),
        other => Err(Error::new(other.span(), "an authoring attribute belongs on the struct or enum the definition is")),
    }
}

/* === Tests === */
