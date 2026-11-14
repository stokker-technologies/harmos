/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Stored Serde Shapes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use serde_json::{Map, Value, json};
use syn::{Attribute, Error, Fields, LitStr, Type, spanned::Spanned};

// Local Imports
use crate::path::HarmosPath;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Attributes {
    rename: Option<String>,
    rename_all: Option<String>,
    rename_all_fields: Option<String>,
    tag: Option<String>,
    content: Option<String>,
    transparent: bool,
    untagged: bool,
    skip: bool,
    omission: Option<String>,
}

struct Shape {
    helpers: Vec<TokenStream>,
    fields: Vec<TokenStream>,
    representation: Map<String, Value>,
    value: Option<(TokenStream, TokenStream)>,
}

/* === Enums === */

#[derive(Clone, Copy)]
enum Location {
    Container,
    Variant,
    Field,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn shape(item: &syn::Item, harmos: &HarmosPath) -> Result<TokenStream, Error> {
    let (name, attributes, generics) = match item {
        syn::Item::Struct(item) => (&item.ident, &item.attrs, &item.generics),
        syn::Item::Enum(item) => (&item.ident, &item.attrs, &item.generics),
        other => return Err(Error::new(other.span(), "a stored shape requires a struct or enum")),
    };
    if !generics.params.is_empty() {
        return Err(Error::new(name.span(), format!("Described type `{name}` requires a non-generic declaration")));
    }
    let owner = name.to_string();
    let attributes = attributes_for(attributes, Location::Container, &owner)?;
    for (attribute, value) in [("tag", &attributes.tag), ("content", &attributes.content)] {
        if value.as_ref().is_some_and(|value| value.len() > 256) {
            return Err(refused(name.span(), attribute, &owner, "keys longer than 256 bytes are unsupported"));
        }
    }
    if let syn::Item::Struct(item) = item {
        if attributes.tag.is_some() || attributes.content.is_some() || attributes.untagged {
            return Err(refused(name.span(), "tag/content/untagged", &owner, "only enum tagging is supported"));
        }
        let mut shape = fields(&item.fields, attributes.rename_all.as_deref(), &owner, &item.ident, attributes.transparent, harmos)?;
        if attributes.transparent {
            if shape.fields.len() != 1 {
                return Err(refused(name.span(), "transparent", &owner, "exactly one serialized field is required"));
            }
            shape.representation.insert("form".into(), json!("value"));
        }
        let newtype = matches!(&item.fields, Fields::Unnamed(fields) if fields.unnamed.len() == 1);
        if attributes.transparent || newtype {
            let field = item
                .fields
                .iter()
                .find(|field| attributes_for(&field.attrs, Location::Field, &owner).is_ok_and(|attributes| !attributes.skip))
                .expect("one serialized field");
            let ty = super::boxed(&field.ty).unwrap_or(&field.ty);
            let container =
                matches!(super::type_name(ty).as_deref(), Some("Vec" | "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "Box"));
            if !container || super::is_bytes(ty) {
                let (kind, cardinality) = shape.value.as_ref().expect("one value encoding");
                return Ok(quote! {
                    impl #harmos::schema::Described for #name {
                        const NAME: &'static str = concat!(module_path!(), "::", stringify!(#name));
                        const FIELDS: &'static [#harmos::schema::graph::Field<#harmos::schema::declaration::Definition>] = <#ty as #harmos::schema::Described>::FIELDS;
                        const FIELD_KIND: #harmos::schema::graph::Kind<#harmos::schema::declaration::Definition> = #kind;
                        const FIELD_CARDINALITY: #harmos::schema::Cardinality = #cardinality;
                        const SERDE: &'static str = <#ty as #harmos::schema::Described>::SERDE;
                        const SERDE_OBJECT: bool = <#ty as #harmos::schema::Described>::SERDE_OBJECT;
                        const SERDE_TAG: ::std::option::Option<&'static str> = <#ty as #harmos::schema::Described>::SERDE_TAG;
                        const MARKER: &'static [u8] = &#harmos::schema::declaration::encode_serde::<{
                            #harmos::schema::declaration::encoded_serde_len(Self::NAME, Self::FIELDS, Self::SERDE)
                        }>(Self::NAME, Self::FIELDS, Self::SERDE);
                    }
                });
            }
        }
        return Ok(declare(name, shape, harmos));
    }
    let syn::Item::Enum(item) = item else {
        unreachable!()
    };
    if attributes.transparent {
        return Err(refused(name.span(), "transparent", &owner, "requires a struct"));
    }
    if attributes.untagged && (attributes.tag.is_some() || attributes.content.is_some())
        || attributes.content.is_some() && attributes.tag.is_none()
        || attributes.tag.is_some() && attributes.tag == attributes.content
    {
        return Err(refused(name.span(), "tag/content/untagged", &owner, "conflicting enum representation"));
    }
    let form = if attributes.untagged {
        "untagged"
    } else if attributes.content.is_some() {
        "adjacent"
    } else if attributes.tag.is_some() {
        "internal"
    } else {
        "external"
    };
    let mut representation = Map::from_iter([("form".into(), json!(form))]);
    if let Some(tag) = &attributes.tag {
        representation.insert("tag".into(), json!(tag));
    }
    if let Some(content) = &attributes.content {
        representation.insert("content".into(), json!(content));
    }
    let mut payloads = Vec::new();
    let mut variants = Vec::new();
    for (index, variant) in item.variants.iter().enumerate() {
        let variant_attributes = attributes_for(&variant.attrs, Location::Variant, &owner)?;
        if variant_attributes.skip {
            continue;
        }
        let field_rule = variant_attributes.rename_all.as_deref().or(attributes.rename_all_fields.as_deref());
        let payload = quote::format_ident!("__Harmos{}Payload{index}", owner.trim_start_matches("r#"));
        let shape = fields(&variant.fields, field_rule, &owner, &payload, false, harmos)?;
        if form == "internal" {
            if matches!(&variant.fields, Fields::Unnamed(fields) if fields.unnamed.len() != 1) {
                return Err(refused(variant.span(), "tag", &owner, "internally tagged tuple variants are unsupported by Serde"));
            }
            if let Fields::Named(fields) = &variant.fields {
                for field in &fields.named {
                    let field_attributes = attributes_for(&field.attrs, Location::Field, &owner)?;
                    let field_name = serialized_name(field.ident.as_ref().unwrap(), field_attributes.rename, field_rule, false);
                    if !field_attributes.skip && Some(&field_name) == attributes.tag.as_ref() {
                        return Err(refused(field.span(), "tag", &owner, "the tag conflicts with a serialized field"));
                    }
                }
            }
            if let Fields::Unnamed(fields) = &variant.fields {
                let ty = &fields.unnamed[0].ty;
                if matches!(super::type_name(ty).as_deref(), Some("BTreeMap" | "HashMap")) {
                    return Err(refused(variant.span(), "tag", &owner, "dynamic map keys can collide with the tag"));
                } else {
                    let diagnostic = format!("Serde attribute `tag` on type `{owner}` requires an object-valued newtype variant");
                    payloads.push(quote!(
                        const _: () = assert!(<#ty as #harmos::schema::Described>::SERDE_OBJECT, #diagnostic);
                    ));
                    let tag = attributes.tag.as_ref().unwrap();
                    let diagnostic = format!("Serde attribute `tag` on type `{owner}` conflicts with a newtype payload field");
                    payloads.push(quote!(
                        const _: () = assert!(#harmos::schema::wire::tag_available(
                        <#ty as #harmos::schema::Described>::FIELDS, <#ty as #harmos::schema::Described>::SERDE_TAG, #tag
                    ), #diagnostic);
                    ));
                }
            }
        }
        let declaration = declare(&payload, shape, harmos);
        payloads.push(quote!(struct #payload; #declaration));
        let variant_name = if attributes.untagged {
            format!("variant_{}", variants.len())
        } else {
            serialized_name(&variant.ident, variant_attributes.rename, attributes.rename_all.as_deref(), true)
        };
        validate_name(&variant_name, variant.span(), &owner)?;
        variants.push(quote!(#harmos::schema::graph::Field {
            name: ::std::borrow::Cow::Borrowed(#variant_name), tag: #harmos::schema::field_tag(#variant_name),
            kind: #harmos::schema::graph::Kind::Message { definition: <#payload as #harmos::schema::Described>::DEFINITION },
            cardinality: #harmos::schema::Cardinality::Variant,
        }));
    }
    let declaration = declare(
        name,
        Shape {
            helpers: Vec::new(),
            fields: variants,
            representation,
            value: None,
        },
        harmos,
    );
    Ok(quote!(const _: () = { #(#payloads)* #declaration };))
}

/* === Private Functions === */

fn refused(span: proc_macro2::Span, attribute: &str, owner: &str, reason: &str) -> Error {
    Error::new(span, format!("Serde attribute `{attribute}` on type `{owner}` cannot be described: {reason}"))
}

fn attributes_for(attributes: &[Attribute], location: Location, owner: &str) -> Result<Attributes, Error> {
    let mut output = Attributes::default();
    for attribute in attributes.iter().filter(|attribute| attribute.path().is_ident("serde")) {
        attribute.parse_nested_meta(|meta| {
            let key = meta.path.to_token_stream().to_string();
            let container = matches!(location, Location::Container);
            let variant = matches!(location, Location::Variant);
            let field = matches!(location, Location::Field);
            match key.as_str() {
                "rename" => output.rename = directional(&meta)?,
                "rename_all" if !field => {
                    output.rename_all = directional(&meta)?;
                    validate_rule(output.rename_all.as_deref(), &meta, owner)?;
                }
                "rename_all_fields" if container => {
                    output.rename_all_fields = directional(&meta)?;
                    validate_rule(output.rename_all_fields.as_deref(), &meta, owner)?;
                }
                "tag" if container => output.tag = Some(meta.value()?.parse::<LitStr>()?.value()),
                "content" if container => output.content = Some(meta.value()?.parse::<LitStr>()?.value()),
                "untagged" if container => output.untagged = true,
                "transparent" if container => output.transparent = true,
                "skip" | "skip_serializing" if field || variant => output.skip = true,
                "skip_serializing_if" if field => output.omission = Some(meta.value()?.parse::<LitStr>()?.value()),
                "default" | "deny_unknown_fields" | "skip_deserializing" | "other" => {}
                "alias" | "deserialize_with" | "from" | "try_from" | "crate" | "expecting" | "borrow" => {}
                "bound" => {
                    let _ = directional(&meta)?;
                    return Ok(());
                }
                _ => return Err(refused(meta.path.span(), &key, owner, "the serialized shape is not statically supported")),
            }
            if meta.input.peek(syn::Token![=]) {
                let _: syn::Expr = meta.value()?.parse()?;
            }
            Ok(())
        })?;
    }
    Ok(output)
}

fn directional(meta: &syn::meta::ParseNestedMeta<'_>) -> Result<Option<String>, Error> {
    if meta.input.peek(syn::Token![=]) {
        return Ok(Some(meta.value()?.parse::<LitStr>()?.value()));
    }
    let mut serialize = None;
    meta.parse_nested_meta(|direction| {
        let value = direction.value()?.parse::<LitStr>()?.value();
        if direction.path.is_ident("serialize") {
            serialize = Some(value);
        } else if !direction.path.is_ident("deserialize") {
            return Err(direction.error("expected serialize or deserialize"));
        }
        Ok(())
    })?;
    Ok(serialize)
}

fn validate_rule(rule: Option<&str>, meta: &syn::meta::ParseNestedMeta<'_>, owner: &str) -> Result<(), Error> {
    if rule.is_some_and(|rule| {
        !matches!(
            rule,
            "lowercase"
                | "UPPERCASE"
                | "PascalCase"
                | "camelCase"
                | "snake_case"
                | "SCREAMING_SNAKE_CASE"
                | "kebab-case"
                | "SCREAMING-KEBAB-CASE"
        )
    }) {
        return Err(refused(meta.path.span(), "rename_all", owner, "unknown Serde rename rule"));
    }
    Ok(())
}

fn validate_name(name: &str, span: proc_macro2::Span, owner: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(refused(span, "rename", owner, "schema names must contain 1 to 256 bytes without control characters"));
    }
    Ok(())
}

fn serialized_name(ident: &syn::Ident, explicit: Option<String>, rule: Option<&str>, variant: bool) -> String {
    if let Some(name) = explicit {
        return name;
    }
    let name = ident.to_string();
    let name = name.strip_prefix("r#").unwrap_or(&name);
    let Some(rule) = rule else {
        return name.to_owned();
    };
    if variant {
        let snake = || {
            let mut output = String::new();
            for (index, character) in name.char_indices() {
                if index > 0 && character.is_uppercase() {
                    output.push('_');
                }
                output.push(character.to_ascii_lowercase());
            }
            output
        };
        match rule {
            "lowercase" => name.to_ascii_lowercase(),
            "UPPERCASE" => name.to_ascii_uppercase(),
            "camelCase" => name
                .chars()
                .enumerate()
                .map(|(index, character)| {
                    if index == 0 {
                        character.to_ascii_lowercase()
                    } else {
                        character
                    }
                })
                .collect(),
            "snake_case" => snake(),
            "SCREAMING_SNAKE_CASE" => snake().to_ascii_uppercase(),
            "kebab-case" => snake().replace('_', "-"),
            "SCREAMING-KEBAB-CASE" => snake().to_ascii_uppercase().replace('_', "-"),
            _ => name.to_owned(),
        }
    } else {
        let pascal = || {
            let mut output = String::new();
            let mut capitalize = true;
            for character in name.chars() {
                if character == '_' {
                    capitalize = true;
                    continue;
                }
                output.push(if capitalize {
                    character.to_ascii_uppercase()
                } else {
                    character
                });
                capitalize = false;
            }
            output
        };
        match rule {
            "UPPERCASE" | "SCREAMING_SNAKE_CASE" => name.to_ascii_uppercase(),
            "PascalCase" => pascal(),
            "camelCase" => pascal()
                .chars()
                .enumerate()
                .map(|(index, character)| {
                    if index == 0 {
                        character.to_ascii_lowercase()
                    } else {
                        character
                    }
                })
                .collect(),
            "kebab-case" => name.replace('_', "-"),
            "SCREAMING-KEBAB-CASE" => name.to_ascii_uppercase().replace('_', "-"),
            _ => name.to_owned(),
        }
    }
}

fn fields(
    fields: &Fields, rule: Option<&str>, owner: &str, scope: &syn::Ident, transparent: bool, harmos: &HarmosPath,
) -> Result<Shape, Error> {
    let form = if transparent {
        "value"
    } else {
        match fields {
            Fields::Unit => "unit",
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => "value",
            Fields::Unnamed(_) => "tuple",
            Fields::Named(_) => "object",
        }
    };
    let mut representation = Map::from_iter([("form".into(), json!(form))]);
    let mut output = Vec::new();
    let mut helpers = Vec::new();
    let mut omissions = Map::new();
    let mut maps = Vec::new();
    let mut value_encoding = None;
    for (position, field) in fields.iter().enumerate() {
        let attributes = attributes_for(&field.attrs, Location::Field, owner)?;
        if attributes.skip {
            if !matches!(fields, Fields::Named(_)) {
                return Err(refused(field.span(), "skip", owner, "skipped positional fields are unsupported"));
            }
            continue;
        }
        let name = if form == "value" {
            "value".to_owned()
        } else {
            field
                .ident
                .as_ref()
                .map(|ident| serialized_name(ident, attributes.rename, rule, false))
                .unwrap_or_else(|| format!("field_{position}"))
        };
        validate_name(&name, field.span(), owner)?;
        let ty = &field.ty;
        if let Some(predicate) = attributes.omission {
            if form != "object" {
                return Err(refused(field.span(), "skip_serializing_if", owner, "conditional positional fields are unsupported"));
            }
            let path: syn::Path =
                syn::parse_str(&predicate).map_err(|_| refused(field.span(), "skip_serializing_if", owner, "invalid predicate path"))?;
            let segments = path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>();
            let rule = match segments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
                ["Option", "is_none"] | ["std" | "core", "option", "Option", "is_none"]
                    if super::type_name(ty).as_deref() == Some("Option") =>
                {
                    "none"
                }
                ["Vec", "is_empty"] | ["std" | "alloc", "vec", "Vec", "is_empty"] if super::type_name(ty).as_deref() == Some("Vec") => {
                    "empty"
                }
                _ => {
                    return Err(refused(
                        field.span(),
                        "skip_serializing_if",
                        owner,
                        "only Option::is_none and Vec::is_empty on their matching types are supported",
                    ));
                }
            };
            omissions.insert(name.clone(), json!(rule));
        }
        if matches!(super::type_name(ty).as_deref(), Some("HashMap" | "BTreeMap")) {
            maps.push(json!(name));
        }
        let (_, mut kind, mut cardinality) = stored_encoding(ty, harmos)
            .map_err(|error| Error::new(field.span(), format!("Described type `{owner}` field `{name}`: {error}")))?;
        if omissions.get(&name).is_some_and(|rule| rule == "none") {
            let Type::Path(path) = ty else {
                unreachable!()
            };
            let syn::PathArguments::AngleBracketed(arguments) = &path.path.segments.last().unwrap().arguments else {
                return Err(super::unsupported(ty));
            };
            let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else {
                return Err(super::unsupported(ty));
            };
            let (_, inner_kind, inner_cardinality) = stored_encoding(inner, harmos)?;
            let helper = quote::format_ident!("__Harmos{}Field{position}", scope.to_string().trim_start_matches("r#"));
            let field = quote!(#harmos::schema::graph::Field {
                name: ::std::borrow::Cow::Borrowed("value"), tag: 1, kind: #inner_kind, cardinality: #inner_cardinality,
            });
            let declaration = declare(
                &helper,
                Shape {
                    helpers: Vec::new(),
                    fields: vec![field],
                    representation: Map::from_iter([("form".into(), json!("value"))]),
                    value: None,
                },
                harmos,
            );
            helpers.push(quote!(struct #helper; #declaration));
            kind = quote!(#harmos::schema::graph::Kind::Message { definition: <#helper as #harmos::schema::Described>::DEFINITION });
            cardinality = quote!(#harmos::schema::Cardinality::Optional);
        }
        if form == "value" {
            value_encoding = Some((kind.clone(), cardinality.clone()));
        }
        let tag = if form == "value" {
            quote!(1)
        } else {
            quote!(#harmos::schema::field_tag(#name))
        };
        output.push(quote!(#harmos::schema::graph::Field {
            name: ::std::borrow::Cow::Borrowed(#name), tag: #tag, kind: #kind, cardinality: #cardinality,
        }));
    }
    if !omissions.is_empty() {
        representation.insert("omissions".into(), Value::Object(omissions));
    }
    if !maps.is_empty() {
        representation.insert("maps".into(), Value::Array(maps));
    }
    if form == "object" && representation.len() == 1 && !output.is_empty() {
        representation.clear();
    }
    Ok(Shape {
        helpers,
        fields: output,
        representation,
        value: value_encoding,
    })
}

fn stored_encoding(ty: &Type, harmos: &HarmosPath) -> Result<(bool, TokenStream, TokenStream), Error> {
    if matches!(super::type_name(ty).as_deref(), Some("BTreeSet" | "HashSet")) {
        let Type::Path(path) = ty else {
            unreachable!()
        };
        let syn::PathArguments::AngleBracketed(arguments) = &path.path.segments.last().unwrap().arguments else {
            return Err(super::unsupported(ty));
        };
        let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else {
            return Err(super::unsupported(ty));
        };
        return stored_encoding(&syn::parse_quote!(Vec<#inner>), harmos);
    }
    if matches!(super::type_name(ty).as_deref(), Some("u8" | "u16" | "i8" | "i16" | "usize" | "isize" | "u128" | "i128" | "char")) {
        return Ok((false, quote!(<#ty as #harmos::schema::Described>::FIELD_KIND), quote!(#harmos::schema::Cardinality::Required)));
    }
    let encoding = super::encoding(ty, harmos)?;
    if matches!(super::type_name(ty).as_deref(), Some("Option" | "Vec")) && !super::is_bytes(ty) {
        let Type::Path(path) = ty else {
            unreachable!()
        };
        let syn::PathArguments::AngleBracketed(arguments) = &path.path.segments.last().unwrap().arguments else {
            unreachable!()
        };
        let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else {
            unreachable!()
        };
        let kind = quote!(const { match <#inner as #harmos::schema::Described>::FIELD_CARDINALITY {
            #harmos::schema::Cardinality::Required => <#inner as #harmos::schema::Described>::FIELD_KIND,
            _ => #harmos::schema::graph::Kind::Message { definition: <#inner as #harmos::schema::Described>::DEFINITION },
        } });
        return Ok((false, kind, encoding.2));
    }
    if encoding.0 && !matches!(super::type_name(ty).as_deref(), Some("Box")) {
        return Ok((
            true,
            quote!(<#ty as #harmos::schema::Described>::FIELD_KIND),
            quote!(<#ty as #harmos::schema::Described>::FIELD_CARDINALITY),
        ));
    }
    Ok(encoding)
}

fn declare(name: &syn::Ident, shape: Shape, harmos: &HarmosPath) -> TokenStream {
    let fields = shape.fields;
    let helpers = shape.helpers;
    let value_encoding = shape.value.map(|(kind, cardinality)| {
        quote! {
            const FIELD_KIND: #harmos::schema::graph::Kind<#harmos::schema::declaration::Definition> = #kind;
            const FIELD_CARDINALITY: #harmos::schema::Cardinality = #cardinality;
        }
    });
    let representation = if shape.representation.is_empty() {
        String::new()
    } else {
        Value::Object(shape.representation.clone()).to_string()
    };
    let object =
        shape.representation.get("form").and_then(Value::as_str).is_none_or(|form| matches!(form, "object" | "internal" | "adjacent"));
    let tag = shape
        .representation
        .get("tag")
        .and_then(Value::as_str)
        .map(|tag| quote!(::std::option::Option::Some(#tag)))
        .unwrap_or_else(|| quote!(::std::option::Option::None));
    let collision = format!("Serde attribute rename/rename_all on type `{name}` produces colliding serialized fields");
    quote! {
        const _: () = {
        #(#helpers)*
        impl #harmos::schema::Described for #name {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!(#name));
            const FIELDS: &'static [#harmos::schema::graph::Field<#harmos::schema::declaration::Definition>] = {
                let fields: &[#harmos::schema::graph::Field<#harmos::schema::declaration::Definition>] = &[#(#fields),*];
                let mut left = 0;
                while left < fields.len() {
                    let mut right = left + 1;
                    while right < fields.len() {
                        assert!(fields[left].tag != fields[right].tag, #collision);
                        right += 1;
                    }
                    left += 1;
                }
                fields
            };
            const SERDE: &'static str = #representation;
            const SERDE_OBJECT: bool = #object;
            const SERDE_TAG: ::std::option::Option<&'static str> = #tag;
            #value_encoding
            const MARKER: &'static [u8] = &#harmos::schema::declaration::encode_serde::<{
                #harmos::schema::declaration::encoded_serde_len(Self::NAME, Self::FIELDS, Self::SERDE)
            }>(Self::NAME, Self::FIELDS, Self::SERDE);
        }
        };
    }
}

/* === Tests === */
