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
use quote::{ToTokens, format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Expr, Fields, Ident, Item, LitInt, LitStr, Meta, Path, Token, Type};
// Local Imports
use crate::path::{self, HarmosPath};

/* === Constants === */
const DEPTH: usize = 4;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Declaration {
    id: Option<LitStr>,
    presentation: crate::presentation::Presentation,
    window: Option<LitInt>,
    track: Option<Tuple>,
    axis: Option<Ident>,
    history: Vec<Path>,
    batch: Option<LitInt>,
    row: Option<Type>,
    harmos: HarmosPath,
}

struct Tuple {
    fields: Vec<Ident>,
    span: proc_macro2::Span,
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
fn expand(attribute: TokenStream, item: &TokenStream) -> Result<TokenStream, Error> {
    let mut declared = declaration(attribute)?;
    let mut parsed: Item = syn::parse2(item.clone())?;
    declared
        .presentation
        .default_label(&named(&parsed)?.to_string());
    let metadata = declared
        .presentation
        .metadata(
            declared
                .id
                .as_ref()
                .map(|id| id.value())
                .as_deref()
                .unwrap_or(""),
            attributes(&parsed),
        )
        .to_string();
    if let Some(row) = &declared.row {
        let Item::Struct(marker) = &parsed else {
            return Err(Error::new(
                parsed.span(),
                "a named stream uses a unit struct",
            ));
        };
        if !matches!(marker.fields, Fields::Unit) || !marker.generics.params.is_empty() {
            return Err(Error::new(
                marker.span(),
                "a named stream uses a non-generic unit struct",
            ));
        }
        if declared.track.is_some() || declared.axis.is_some() || declared.batch.is_some() {
            return Err(Error::new(
                marker.span(),
                "named plugin streams declare their row shape without native track, axis, or batch policies",
            ));
        }
        let id = declared
            .id
            .as_ref()
            .ok_or_else(|| Error::new(marker.span(), "a named stream requires id and window"))?;
        let window = declared
            .window
            .as_ref()
            .ok_or_else(|| Error::new(marker.span(), "a named stream requires id and window"))?;
        let name = &marker.ident;
        let harmos = &declared.harmos;
        let version = declared.history.len() as u32 + 1;
        let history = &declared.history;
        return Ok(quote! {
            #item
            impl #harmos::StreamSpec for #name {
                type Row = #row;
                const ID: &'static str = #id;
                const VERSION: u32 = #version;
                const WINDOW: usize = #window;
                const METADATA_JSON: &'static str = #metadata;
                const ROW_METADATA_JSON: &'static str = <#row as #harmos::RowMetadata>::ROW_METADATA_JSON;
            }
            const _: () = {
                fn historical_shapes() {
                    #(let _ = <#history as #harmos::schema::Described>::FIELDS;)*
                }
            };
        });
    }

    if declared.batch.is_some() && (declared.track.is_some() || declared.id.is_none()) {
        return Err(Error::new(
            parsed.span(),
            "batch is supported only on current, untracked streams",
        ));
    }
    let batching = super::batch::expand(&mut parsed, declared.batch.as_ref(), &declared.harmos)?;
    let mut presentation_shape = parsed.clone();
    let row_metadata = if let Item::Struct(shape) = &mut presentation_shape {
        crate::presentation::fields(&mut shape.fields)?
    } else {
        "[]".to_owned()
    };
    let explicit_message = match &parsed {
        Item::Struct(shape) => shape.attrs.iter().any(|attr| {
            attr.path()
                .segments
                .last()
                .is_some_and(|part| part.ident == "message")
                || attr.path().is_ident("prost")
        }),
        _ => false,
    };
    let portable = explicit_message || crate::message::portable(&parsed, &declared.harmos);
    let mut item = parsed.to_token_stream();
    if matches!(&parsed, Item::Struct(shape) if matches!(shape.fields, Fields::Named(_)) && shape.generics.params.is_empty())
    {
        let Item::Struct(shape) = &mut parsed else {
            unreachable!()
        };
        if !explicit_message {
            let harmos = &declared.harmos;
            let derives = shape
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("derive"))
                .map(|attr| attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated))
                .collect::<Result<Vec<_>, _>>()?;
            let has = |name: &str| {
                derives.iter().flatten().any(|path| {
                    path.segments
                        .last()
                        .is_some_and(|segment| segment.ident == name)
                })
            };
            let mut added = Vec::<TokenStream>::new();
            if !has("Clone") {
                added.push(quote!(Clone));
            }
            if portable && !has("PartialEq") {
                added.push(quote!(PartialEq));
            }
            if !has("Serialize") {
                added.push(quote!(#harmos::__private::serde::Serialize));
            }
            if !has("Deserialize") {
                added.push(quote!(#harmos::__private::serde::Deserialize));
            }
            if !added.is_empty() {
                shape.attrs.push(syn::parse_quote!(#[derive(#(#added),*)]));
            }
            if !has("Serialize") && !has("Deserialize") {
                let serde_path = LitStr::new(
                    &format!("{}::__private::serde", harmos.to_token_stream()).replace(' ', ""),
                    shape.span(),
                );
                shape
                    .attrs
                    .push(syn::parse_quote!(#[serde(crate = #serde_path)]));
            }
            let mut attributes = Vec::new();
            for attribute in std::mem::take(&mut shape.attrs) {
                if attribute.path().is_ident("derive") {
                    let paths = attribute
                        .parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
                    let paths: Vec<_> = paths
                        .into_iter()
                        .filter(|path| {
                            !portable
                                || !path.segments.last().is_some_and(|part| {
                                    part.ident == "Debug" || part.ident == "Default"
                                })
                        })
                        .collect();
                    if !paths.is_empty() {
                        attributes.push(syn::parse_quote!(#[derive(#(#paths),*)]));
                    }
                } else {
                    attributes.push(attribute);
                }
            }
            shape.attrs = attributes;
            if portable {
                item = crate::message::expand(
                    quote!(harmos_path = #harmos),
                    parsed.to_token_stream(),
                )?;
            } else {
                crate::presentation::fields(&mut shape.fields)?;
                let name = &shape.ident;
                item = quote! {
                    #shape
                    impl #harmos::RowMetadata for #name {
                        const ROW_METADATA_JSON: &'static str = #row_metadata;
                    }
                };
            }
        }
    } else {
        item = presentation_shape.to_token_stream();
    }
    let row_metadata = if portable {
        let harmos = &declared.harmos;
        quote!(<Self as #harmos::RowMetadata>::ROW_METADATA_JSON)
    } else {
        quote!(#row_metadata)
    };
    let name = named(&parsed)?;
    let batch_definition = declared.batch.as_ref().map(|_| {
        let harmos = &declared.harmos;
        quote! { fn batch_definition() -> Option<#harmos::__private::BatchDefinition> { Some(#harmos::__private::BatchDefinition::of::<Self>()) } }
    });

    let Some(id) = &declared.id else {
        if let Some(window) = &declared.window {
            return Err(Error::new(
                window.span(),
                "a current stream row declares `id = \"…\"` and `window = …`",
            ));
        }
        if let Some(coordinate) = declared
            .track
            .as_ref()
            .map(|track| track.span)
            .or_else(|| declared.axis.as_ref().map(Spanned::span))
        {
            return Err(Error::new(
                coordinate,
                "a historical stream row carries no track or axis; the current row owns both coordinates",
            ));
        }
        if !declared.history.is_empty() {
            return Err(Error::new(
                parsed.span(),
                "a historical stream row carries no history of its own; the current row owns the catalog",
            ));
        }

        return Ok(item.clone());
    };

    let Some(window) = &declared.window else {
        return Err(Error::new(
            id.span(),
            "a current stream row declares `id = \"…\"` and `window = …`",
        ));
    };
    let count = declared.history.len();
    let version = LitInt::new(&(count + 1).to_string(), parsed.span());
    let encoded_id: String = id
        .value()
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let id_guard = format_ident!("__HARMOS_STREAM_ID_{}", encoded_id);
    let historical_versions: Vec<LitInt> = (1..=count)
        .map(|version| LitInt::new(&version.to_string(), parsed.span()))
        .collect();
    let history = declared
        .history
        .iter()
        .zip(&historical_versions)
        .map(|(_, version)| quote!((#id, #version)));
    let historical = &declared.history;
    let harmos = &declared.harmos;
    let tracked = match &declared.track {
        Some(track) => {
            let named: Vec<String> = track.fields.iter().map(Ident::to_string).collect();
            let components = track
                .fields
                .iter()
                .map(|field| declared_field(&parsed, field, "track"))
                .collect::<Result<Vec<_>, _>>()?;
            let fields = &track.fields;

            quote! {
                #[automatically_derived]
                impl #harmos::streams::Tracked for #name {
                    const TRACK: &'static [&'static str] = &[#(#named),*];

                    type Track = (#(#components,)*);

                    fn track(&self) -> Self::Track {
                        (#(::core::clone::Clone::clone(&self.#fields),)*)
                    }
                }
            }
        }
        None => quote!(),
    };
    let axial = match &declared.axis {
        Some(field) => {
            let named = field.to_string();
            // The field's own type answers for the coordinate: a field that is
            // not a `u64` fails to compile here rather than being coerced.
            declared_field(&parsed, field, "axis")?;

            quote! {
                #[automatically_derived]
                impl #harmos::streams::Axial for #name {
                    const AXIS: &'static str = #named;

                    fn axis(&self) -> u64 {
                        self.#field
                    }
                }
            }
        }
        None => quote!(),
    };

    Ok(quote! {
        #item

        #tracked
        #axial
        #batching

        #[automatically_derived]
        impl #harmos::StreamSpec for #name {
            type Row = Self;
            const ID: &'static str = #id;
            const VERSION: u32 = #version;
            const WINDOW: usize = #window;
            const METADATA_JSON: &'static str = #metadata;
            const ROW_METADATA_JSON: &'static str = #row_metadata;
        }

        #[automatically_derived]
        impl #harmos::__private::StreamRow for #name {
            const ID: &'static str = #id;
            const VERSION: u32 = #version;
            const WINDOW: usize = #window;
            #batch_definition
            fn metadata() -> #harmos::Metadata { <Self as #harmos::StreamSpec>::metadata() }
            fn row_metadata() -> Vec<#harmos::StreamField> { <Self as #harmos::StreamSpec>::row_metadata() }
        }

        impl #name {

            pub const DEFINITION: (&'static str, u32, usize) = (#id, #version, #window);


            pub const ID: &'static str = #id;


            pub const VERSION: u32 = #version;


            pub const WINDOW: usize = #window;


            pub fn definition() -> #harmos::streams::Definition {
                #harmos::__private::stream_definition::<Self>(#id, #version, #window)
            }


            pub const HISTORY: [(&'static str, u32); #count] = [#(#history),*];




            pub fn restore<'de, D>(
                version: u32,
                payload: D,
            ) -> ::core::result::Result<::core::option::Option<Self>, D::Error>
            where
                D: #harmos::__private::serde::Deserializer<'de> + ::core::marker::Copy,
            {
                if version == #version {
                    return ::core::result::Result::Ok(::core::option::Option::Some(
                        #harmos::__private::serde::Deserialize::deserialize(payload)?,
                    ));
                }
                #(
                    if version == #historical_versions {
                        let historical: #historical =
                            #harmos::__private::serde::Deserialize::deserialize(payload)?;

                        return ::core::result::Result::Ok(::core::option::Option::Some(
                            ::core::convert::TryFrom::try_from(historical)
                                .map_err(#harmos::__private::serde::de::Error::custom)?,
                        ));
                    }
                )*

                ::core::result::Result::Ok(::core::option::Option::None)
            }
        }

        const _: () = {
            fn stream_rows_are_serde<T>()
            where
                T: #harmos::__private::serde::Serialize + #harmos::__private::serde::de::DeserializeOwned,
            {
            }

            fn this_stream_row_is_serde() {
                stream_rows_are_serde::<#name>();
            }
        };


        const #id_guard: () = ();
    })
}

fn attributes(item: &Item) -> &[syn::Attribute] {
    match item {
        Item::Struct(shape) => &shape.attrs,
        Item::Enum(shape) => &shape.attrs,
        Item::Type(shape) => &shape.attrs,
        _ => &[],
    }
}

fn declaration(attribute: TokenStream) -> Result<Declaration, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut id = None;
    let mut presentation = crate::presentation::Presentation::default();
    let mut window = None;
    let mut track = None;
    let mut axis = None;
    let mut history = Vec::new();
    let mut batch = None;
    let mut row = None;
    let mut harmos_path: Option<Path> = None;

    for argument in &arguments {
        if presentation.parse(argument, false)? {
            continue;
        }
        if let Some(parsed) = path::declared(argument) {
            set_once(&mut harmos_path, parsed?, argument.span(), path::KEY)?;
            continue;
        }
        match argument {
            Meta::NameValue(named) if named.path.is_ident("id") => {
                set_once(
                    &mut id,
                    syn::parse2::<LitStr>(named.value.to_token_stream())?,
                    named.span(),
                    "id",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("window") => {
                set_once(
                    &mut window,
                    syn::parse2::<LitInt>(named.value.to_token_stream())?,
                    named.span(),
                    "window",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("row") => {
                set_once(
                    &mut row,
                    syn::parse2::<Type>(named.value.to_token_stream())?,
                    named.span(),
                    "row",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("track") => {
                set_once(&mut track, tuple(&named.value)?, named.span(), "track")?;
            }
            Meta::NameValue(named) if named.path.is_ident("axis") => {
                set_once(
                    &mut axis,
                    syn::parse2::<Ident>(named.value.to_token_stream())?,
                    named.span(),
                    "axis",
                )?;
            }
            Meta::List(listed) if listed.path.is_ident("batch") => {
                set_once(
                    &mut batch,
                    super::batch::samples(listed)?,
                    listed.span(),
                    "batch",
                )?;
            }
            Meta::List(listed) if listed.path.is_ident("history") => {
                if !history.is_empty() {
                    return Err(Error::new(listed.span(), "`history` is declared only once"));
                }
                history = super::catalogued(listed)?;
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "a stream declares `id = \"…\"`, `window = …`, and optionally `track = (field, …)`, `axis = field`, `history(…)`, and `harmos_path = …`",
                ));
            }
        }
    }

    if let Some(id) = &id {
        validate_id(id)?;
    }
    if let Some(window) = &window {
        positive_usize(window, "a stream window must retain at least 1 row")?;
    }
    if let (Some(track), Some(axis)) = (&track, &axis)
        && let Some(both) = track.fields.iter().find(|field| *field == axis)
    {
        return Err(Error::new(
            both.span(),
            "a field is a track component or the axis, never both: one places rows and the other partitions them",
        ));
    }

    Ok(Declaration {
        id,
        presentation,
        window,
        track,
        axis,
        history,
        batch,
        row,
        harmos: HarmosPath::resolve(harmos_path),
    })
}

fn tuple(value: &Expr) -> Result<Tuple, Error> {
    let span = value.span();
    let fields = match value {
        Expr::Tuple(listed) => listed
            .elems
            .iter()
            .map(component)
            .collect::<Result<Vec<_>, _>>()?,
        Expr::Paren(single) => vec![component(&single.expr)?],
        other => {
            return Err(Error::new(
                other.span(),
                "a track is declared as a tuple of the row's own fields: `track = (run, probe)`",
            ));
        }
    };

    if fields.is_empty() {
        return Err(Error::new(
            span,
            "a declared track names at least one field; a stream that partitions by nothing declares no track",
        ));
    }
    if fields.len() > DEPTH {
        return Err(Error::new(
            span,
            format!("a declared track names at most {DEPTH} fields"),
        ));
    }
    for (at, field) in fields.iter().enumerate() {
        if fields[..at].contains(field) {
            return Err(Error::new(
                field.span(),
                "a declared track names each field once; a repeated component partitions by nothing new",
            ));
        }
    }

    Ok(Tuple { fields, span })
}

fn component(value: &Expr) -> Result<Ident, Error> {
    match value {
        Expr::Path(path) => path.path.get_ident().cloned().ok_or_else(|| {
            Error::new(
                path.span(),
                "a track component is one field of this row, named plainly",
            )
        }),
        other => Err(Error::new(
            other.span(),
            "a track component is one field of this row, named plainly",
        )),
    }
}

fn declared_field<'row>(
    row: &'row Item,
    field: &Ident,
    coordinate: &str,
) -> Result<&'row Type, Error> {
    let fields = match row {
        Item::Struct(shape) => &shape.fields,
        other => {
            return Err(Error::new(
                other.span(),
                format!("a declared `{coordinate}` names a field, so the row is a struct"),
            ));
        }
    };
    let Fields::Named(named) = fields else {
        return Err(Error::new(
            fields.span(),
            format!("a declared `{coordinate}` names a field, so the row has named fields"),
        ));
    };

    named
        .named
        .iter()
        .find(|candidate| candidate.ident.as_ref() == Some(field))
        .map(|candidate| &candidate.ty)
        .ok_or_else(|| {
            Error::new(
                field.span(),
                format!("the declared `{coordinate}` names no field of this row"),
            )
        })
}

fn set_once<T>(
    place: &mut Option<T>,
    value: T,
    span: proc_macro2::Span,
    name: &str,
) -> Result<(), Error> {
    if place.replace(value).is_some() {
        return Err(Error::new(span, format!("`{name}` is declared only once")));
    }

    Ok(())
}

fn positive_usize(literal: &LitInt, message: &str) -> Result<(), Error> {
    if !matches!(literal.suffix(), "" | "usize") {
        return Err(Error::new(
            literal.span(),
            "a stream window is an unsuffixed integer or `usize`",
        ));
    }
    let value = literal
        .base10_parse::<usize>()
        .map_err(|error| Error::new(literal.span(), error))?;
    if value == 0 {
        return Err(Error::new(literal.span(), message));
    }

    Ok(())
}

fn validate_id(id: &LitStr) -> Result<(), Error> {
    let value = id.value();
    let safe = value.split('.').all(|segment| {
        let mut characters = segment.chars();
        characters
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && characters.all(|character| {
                character == '_' || character == '-' || character.is_ascii_alphanumeric()
            })
            && !python_keyword(segment)
            && !filesystem_reserved(segment)
    });
    if !safe {
        return Err(Error::new(
            id.span(),
            "a stream id must be a filesystem-safe dotted name with non-keyword ASCII segments; use letters, digits, `_`, and `-`, starting each segment with a letter or `_`",
        ));
    }
    Ok(())
}

fn filesystem_reserved(value: &str) -> bool {
    let uppercase = value.to_ascii_uppercase();

    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || uppercase.strip_prefix("COM").is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || uppercase.strip_prefix("LPT").is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

fn python_keyword(value: &str) -> bool {
    matches!(
        value,
        "False"
            | "None"
            | "True"
            | "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

fn named(item: &Item) -> Result<&Ident, Error> {
    match item {
        Item::Struct(shape) => Ok(&shape.ident),
        Item::Enum(shape) => Ok(&shape.ident),
        other => Err(Error::new(
            other.span(),
            "a stream attribute belongs on the struct or enum the row is",
        )),
    }
}

/* === Tests === */
