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
use proc_macro2::TokenStream;
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
    if sort == Sort::Transaction && declared.undo.is_none() {
        return Err(Error::new(
            declared.version.span(),
            "a transaction declares what undoes it: `inverse = …` names the change that takes \
             it back, `inverse = Self` says it is its own, and `irreversible` says it cannot be \
             taken back",
        ));
    }
    let id = declared.id.as_ref().ok_or_else(|| Error::new(parsed.span(), "a definition requires id"))?;
    let harmos = &declared.harmos;
    let kind = sort.kind(harmos);
    let restore = restore();
    let undoing = declared.undo.as_ref().map(|undo| undoing(name, undo, harmos)).unwrap_or_default();

    let record_marker = match sort {
        Sort::Record => quote! { impl #harmos::state::Record for #name {} },
        Sort::Transaction => quote! {
            impl<S> #harmos::state::TransactionCatalog<S> for #name
            where Self: #harmos::state::Transaction<S>, S: 'static {
                fn replay(&self, prepared: &#harmos::state::Prepared, state: &mut S) -> ::core::result::Result<(), #harmos::state::DecodeError> {
                    let refusal = || #harmos::state::DecodeError::Malformed { id: prepared.id.clone(), version: prepared.version };
                    if prepared.id.as_str() != Self::DEFINITION.1 { return Err(refusal()); }
                    let value = Self::restore(prepared.version, &prepared.payload)
                        .map_err(|_| refusal())?.ok_or_else(refusal)?;
                    #harmos::state::Transaction::apply(&value, state);
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

fn restore() -> TokenStream {
    quote! {
        pub fn restore<'de, D>(_version: u32, payload: D) -> ::core::result::Result<::core::option::Option<Self>, D::Error>
        where D: ::serde::Deserializer<'de> + ::core::marker::Copy {
            Ok(Some(::serde::Deserialize::deserialize(payload)?))
        }
    }
}

fn declaration(attribute: TokenStream, sort: Sort) -> Result<Declaration, Error> {
    let span = attribute.span();
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut id = None;
    let version = LitInt::new("1", span);
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
                        "a {} declares `id = \"…\"`, the current v1 shape{}, and may \
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

    Ok(Declaration { id, version, undo, harmos: HarmosPath::resolve(harmos_path) })
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

fn named(item: &Item) -> Result<&Ident, Error> {
    match item {
        Item::Struct(shape) => Ok(&shape.ident),
        Item::Enum(shape) => Ok(&shape.ident),
        other => Err(Error::new(other.span(), "an authoring attribute belongs on the struct or enum the definition is")),
    }
}

/* === Tests === */
