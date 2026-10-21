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
use std::collections::BTreeMap;
// Third Party Imports
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, Fields, ItemEnum, spanned::Spanned};
// Local Imports
use super::{encoding, tag};
use crate::path::HarmosPath;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// A Message Enum Is A Oneof: Each Variant Selects One Payload Shape Whose Fields
// Are The Variant's Fields. Payload Shapes Are Unit Markers Carrying Only Their
// Described Fields; Values Live In The Enum And Its Postcard Encoding.
pub(super) fn expand(mut item: ItemEnum, harmos: HarmosPath, skip_debug: bool) -> Result<TokenStream, Error> {
    if !item.generics.params.is_empty() || item.variants.is_empty() {
        return Err(Error::new(item.span(), "a message enum requires non-generic variants"));
    }
    let postcard = super::postcard::declare(&syn::Item::Enum(item.clone()), &harmos);
    let name = &item.ident;
    let mut tags = BTreeMap::new();
    let mut payloads = Vec::new();
    let mut memories = Vec::new();
    let mut schemas = Vec::new();
    let mut defaults = Vec::new();
    let mut defaultable = true;
    for (index, variant) in item.variants.iter_mut().enumerate() {
        if variant.discriminant.is_some() {
            return Err(Error::new(variant.span(), "message variants use name-derived tags, without discriminants"));
        }
        let variant_name = &variant.ident;
        let authored = variant_name.to_string();
        let authored = authored.strip_prefix("r#").unwrap_or(&authored);
        let number = tag(authored);
        if let Some(previous) = tags.insert(number, authored.to_owned()) {
            return Err(Error::new(
                variant.span(),
                format!("message variants `{previous}` and `{authored}` collide at protobuf tag {number}; rename a variant explicitly"),
            ));
        }
        let payload = format_ident!("{name}Payload{index}");
        let mut field_tags = BTreeMap::new();
        let mut fields = Vec::new();
        let mut bindings = Vec::new();
        let mut members = Vec::new();
        let mut owned = Vec::new();
        let mut initial = Vec::new();
        for (position, field) in variant.fields.iter_mut().enumerate() {
            let ident = field.ident.clone().unwrap_or_else(|| format_ident!("field_{position}"));
            // Validate The Same Portable Field Shapes As Struct Messages
            let (required_message, kind, cardinality) = encoding(&field.ty, &harmos)?;
            if index == 0 && required_message {
                defaultable = false;
            }
            let field_name = ident.to_string();
            let field_name = field_name.strip_prefix("r#").unwrap_or(&field_name);
            let field_tag = tag(field_name);
            if let Some(previous) = field_tags.insert(field_tag, field_name.to_owned()) {
                return Err(Error::new(
                    field.span(),
                    format!(
                        "message fields `{previous}` and `{field_name}` collide at protobuf tag {field_tag}; rename a field explicitly"
                    ),
                ));
            }
            fields.push(quote! {
                #harmos::schema::graph::Field {
                    name: ::std::borrow::Cow::Borrowed(#field_name), tag: #field_tag,
                    kind: #kind, cardinality: #cardinality,
                }
            });
            // Row Presentation Describes Struct Rows; A Variant Field Has None
            field.attrs.retain(|attribute| !attribute.path().is_ident("row"));
            let binding = format_ident!("__harmos_field_{position}");
            owned.push(quote! {
                let owned = #harmos::MemorySize::memory_size(#binding);
                if owned == usize::MAX { return usize::MAX; }
                let bytes = bytes.saturating_add(owned.saturating_sub(::std::mem::size_of_val(#binding)));
            });
            bindings.push(binding);
            members.push(ident);
            initial.push(quote!(::std::default::Default::default()));
        }
        let pattern = match &variant.fields {
            Fields::Unit => quote!(#name::#variant_name),
            Fields::Unnamed(_) => quote!(#name::#variant_name(#(#bindings),*)),
            Fields::Named(_) => quote!(#name::#variant_name { #(#members: #bindings),* }),
        };
        defaults.push(match &variant.fields {
            Fields::Unit => quote!(#name::#variant_name),
            Fields::Unnamed(_) => quote!(#name::#variant_name(#(#initial),*)),
            Fields::Named(_) => quote!(#name::#variant_name { #(#members: #initial),* }),
        });
        let described = super::described(&payload, quote!(&[#(#fields),*]), quote!(), &harmos);
        payloads.push(quote! {
            struct #payload;
            #described
        });
        memories.push(quote! { #pattern => { let bytes = ::std::mem::size_of::<Self>(); #(#owned)* bytes } });
        schemas.push(quote! { #harmos::schema::graph::Field {
            name: ::std::borrow::Cow::Borrowed(#authored), tag: #number,
            kind: #harmos::schema::graph::Kind::Message { definition: <#payload as #harmos::schema::Described>::DEFINITION },
            cardinality: #harmos::schema::Cardinality::Variant,
        }});
    }
    let derives: Vec<_> = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .map(|attr| attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    let has_default = derives.iter().any(|path| path.segments.last().is_some_and(|part| part.ident == "Default"));
    // An Absent Oneof Decodes To The `#[default]` Unit Variant, Else The First.
    let default_variant = item.variants.iter().position(|variant| {
        has_default && matches!(variant.fields, Fields::Unit) && variant.attrs.iter().any(|attribute| attribute.path().is_ident("default"))
    });
    if !skip_debug && !derives.iter().any(|path| path.segments.last().is_some_and(|part| part.ident == "Debug")) {
        item.attrs.push(syn::parse_quote!(#[derive(Debug)]));
    }
    let default = &defaults[0];
    let default_impl = (!has_default && defaultable).then(|| {
        quote! {
            impl ::std::default::Default for #name { fn default() -> Self { #default } }
        }
    });
    let extra =
        default_variant.map(|index| quote!(const DEFAULT_VARIANT: ::std::option::Option<usize> = ::std::option::Option::Some(#index);));
    let schemas = schemas.into_iter();
    let schema = super::described(name, quote!(&[#(#schemas),*]), quote!(#extra), &harmos);
    Ok(quote! {
        #item
        #postcard
        const _: () = {
            #(#payloads)*
            #default_impl
            impl #harmos::MemorySize for #name {
                fn memory_size(&self) -> usize {
                    match self { #(#memories),* }
                }
            }
            impl #harmos::RowMetadata for #name { const ROW_METADATA_JSON: &'static str = "[]"; }
            #schema
        };
    })
}

/* === Private Functions === */

/* === Tests === */
