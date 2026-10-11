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
use quote::quote;
use syn::parse::{ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Error, Ident, ItemEnum, LitInt, Meta, Path, Token, parenthesized};
// Local Imports
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Declared {
    name: Ident,
    capacity: Capacity,
    default: bool,
}

/* === Enums === */
enum Capacity {
    Serial,
    Concurrent(LitInt),
    Unbounded,
}

/* === Traits === */

/* === Implementations === */
impl Declared {
    fn capacity(&self, harmos: &HarmosPath) -> TokenStream {
        match &self.capacity {
            Capacity::Serial => quote! { #harmos::Capacity::serial() },
            Capacity::Concurrent(width) => quote! { #harmos::Capacity::concurrent(#width) },
            Capacity::Unbounded => quote! { #harmos::Capacity::unbounded() },
        }
    }
}

/* === Public Functions === */
pub(crate) fn close(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, item) {
        Ok(closed) => closed,
        Err(refused) => refused.to_compile_error(),
    }
}

/* === Private Functions === */
fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let harmos = harmos_path(attribute)?;

    let mut catalog: ItemEnum = syn::parse2(item)?;
    let lanes = declarations(&catalog)?;
    strip(&mut catalog);

    let name = &catalog.ident;
    let visibility = &catalog.vis;
    let names = lanes.iter().map(|lane| {
        let variant = &lane.name;
        let text = variant.to_string();

        quote!(Self::#variant => #text,)
    });
    let capacities = lanes.iter().map(|lane| {
        let variant = &lane.name;
        let capacity = lane.capacity(&harmos);

        quote!(Self::#variant => #capacity,)
    });
    let entries = lanes.iter().map(|lane| {
        let variant = &lane.name;

        quote! {
            (
                #harmos::Lane::name(&Self::#variant),
                #harmos::Lane::capacity(&Self::#variant),
            )
        }
    });
    let fallback = &lanes
        .iter()
        .find(|lane| lane.default)
        .expect("one default lane is declared")
        .name;

    Ok(quote! {
        #[derive(Clone, Copy, Debug)]
        #catalog

        #[automatically_derived]
        impl ::core::default::Default for #name {
            fn default() -> Self {
                Self::#fallback
            }
        }

        #[automatically_derived]
        impl #harmos::Lane for #name {
            fn name(&self) -> &'static str {
                match self { #(#names)* }
            }

            fn capacity(&self) -> #harmos::Capacity {
                match self { #(#capacities)* }
            }
        }

        impl #name {

            #visibility fn catalog() -> #harmos::LaneSet {
                #harmos::LaneSet::declared([#(#entries),*])
            }
        }
    })
}

fn harmos_path(attribute: TokenStream) -> Result<HarmosPath, Error> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut harmos_path: Option<Path> = None;

    for argument in &arguments {
        match path::declared(argument) {
            Some(parsed) => set_once(&mut harmos_path, parsed?, argument.span(), path::KEY)?,
            None => {
                return Err(Error::new(
                    argument.span(),
                    "a lane catalog declares no arguments of its own; each lane declares its \
                     own capacity, and a lane catalog re-exported through a facade crate \
                     declares `harmos_path = …`",
                ));
            }
        }
    }

    Ok(HarmosPath::resolve(harmos_path))
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

fn declarations(catalog: &ItemEnum) -> Result<Vec<Declared>, Error> {
    let mut lanes = Vec::new();

    for variant in &catalog.variants {
        if !matches!(variant.fields, syn::Fields::Unit) {
            return Err(Error::new(
                variant.span(),
                "a lane is a bare variant: it names one lane and carries no payload",
            ));
        }

        let mut declared = variant.attrs.iter().filter(|attribute| is_lane(attribute));
        let fallback = variant.attrs.iter().find(|attribute| is_default(attribute));
        let capacity = match (fallback, declared.next()) {
            (Some(fallback), Some(_)) => {
                return Err(Error::new(
                    fallback.span(),
                    "the default lane is unbounded, so it declares no capacity of its own: \
                     a job that named no lane must start at once, and a job that should \
                     queue names the lane it queues for",
                ));
            }
            // Always unbounded, and never spelled. Every job that names no
            // lane lands here, so a width would be one number bounding work
            // nobody counted — the back-pressure a declaration promises is
            // the promise a default lane cannot keep.
            (Some(_), None) => Capacity::Unbounded,
            (None, Some(attribute)) => capacity(attribute)?,
            (None, None) => {
                return Err(Error::new(
                    variant.span(),
                    "a lane declares its capacity: `#[lane(serial)]`, `#[lane(concurrent(n))]`, \
                     or `#[lane(unbounded)]`; the one lane marked `#[default]` declares none",
                ));
            }
        };
        if let Some(second) = declared.next() {
            return Err(Error::new(second.span(), "a lane declares one capacity"));
        }

        lanes.push(Declared {
            name: variant.ident.clone(),
            capacity,
            default: fallback.is_some(),
        });
    }

    if lanes.is_empty() {
        return Err(Error::new(
            catalog.span(),
            "a lane catalog declares at least one lane",
        ));
    }
    let mut defaults = lanes.iter().filter(|lane| lane.default);
    if defaults.next().is_none() {
        return Err(Error::new(
            catalog.span(),
            "a lane catalog marks exactly one lane `#[default]`: it is where every job that \
             names no lane of its own lands, and an orchestrator names none",
        ));
    }
    if let Some(second) = defaults.next() {
        return Err(Error::new(
            second.name.span(),
            "a lane catalog marks exactly one lane `#[default]`",
        ));
    }

    Ok(lanes)
}

fn capacity(attribute: &Attribute) -> Result<Capacity, Error> {
    if !matches!(attribute.meta, Meta::List(_)) {
        return Err(Error::new(
            attribute.span(),
            "a lane's capacity is `#[lane(serial)]`, `#[lane(concurrent(n))]`, \
             or `#[lane(unbounded)]`",
        ));
    }

    attribute.parse_args_with(|input: ParseStream| {
        let word: Ident = input.parse()?;
        let capacity = match word.to_string().as_str() {
            "serial" => Capacity::Serial,
            "unbounded" => Capacity::Unbounded,
            "concurrent" => {
                let width;
                parenthesized!(width in input);
                let width: LitInt = width.parse()?;
                if width.base10_parse::<usize>()? == 0 {
                    return Err(Error::new(
                        width.span(),
                        "a lane must admit at least one job",
                    ));
                }
                Capacity::Concurrent(width)
            }
            _ => {
                return Err(Error::new(
                    word.span(),
                    "a lane's capacity is `serial`, `concurrent(n)`, or `unbounded`",
                ));
            }
        };
        if !input.is_empty() {
            return Err(input.error("a lane declares one capacity"));
        }

        Ok(capacity)
    })
}

fn strip(catalog: &mut ItemEnum) {
    for variant in &mut catalog.variants {
        variant
            .attrs
            .retain(|attribute| !is_lane(attribute) && !is_default(attribute));
    }
}

fn is_lane(attribute: &Attribute) -> bool {
    attribute.path().is_ident("lane")
}

fn is_default(attribute: &Attribute) -> bool {
    matches!(attribute.meta, Meta::Path(_)) && attribute.path().is_ident("default")
}

/* === Tests === */
