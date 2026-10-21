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
use quote::ToTokens;
use serde_json::{Value, json};
use syn::{
    Error, Expr, Fields, Lit, LitStr, Meta, Token,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
pub(crate) struct Presentation {
    label: Option<String>,
    description: Option<LitStr>,
    tags: Option<Vec<String>>,
    attributes: Option<serde_json::Map<String, Value>>,
    pub(crate) role: Option<String>,
    pub(crate) unit: Option<String>,
}

struct Attribute {
    key: String,
    span: proc_macro2::Span,
    value: Value,
}

struct AttributeValue(Value);

struct ObjectMember(Attribute);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Parse for Attribute {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let (key, span) = key(input)?;
        input.parse::<Token![=]>()?;
        let value = input.parse::<AttributeValue>()?.0;

        Ok(Self { key, span, value })
    }
}

impl Parse for ObjectMember {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let (key, span) = key(input)?;
        input.parse::<Token![:]>()?;
        let value = input.parse::<AttributeValue>()?.0;

        Ok(Self(Attribute { key, span, value }))
    }
}

impl Parse for AttributeValue {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(syn::token::Bracket) {
            let content;
            syn::bracketed!(content in input);
            let values = Punctuated::<Self, Token![,]>::parse_terminated(&content)?;
            let values: Vec<_> = values.into_iter().map(|value| value.0).collect();

            return Ok(Self(json!({"type": "array", "value": values})));
        }
        if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            let members = Punctuated::<ObjectMember, Token![,]>::parse_terminated(&content)?;
            let object = attributes(members.into_iter().map(|member| member.0))?;

            return Ok(Self(json!({"type": "object", "value": object})));
        }
        let expression: Expr = input.parse()?;
        if matches!(&expression, Expr::Path(path) if path.path.is_ident("null")) {
            return Ok(Self(json!({"type": "null"})));
        }

        literal(&expression).map(Self)
    }
}

impl Presentation {
    pub(crate) fn default_label(&mut self, name: &str) {
        if self.label.is_none() {
            self.label = Some(title(name));
        }
    }

    pub(crate) fn parse(&mut self, meta: &Meta, row: bool) -> Result<bool, Error> {
        match meta {
            Meta::NameValue(pair) if pair.path.is_ident("label") => set(&mut self.label, text(&pair.value)?, meta)?,
            Meta::NameValue(pair) if pair.path.is_ident("description") => {
                let written = syn::parse2::<LitStr>(pair.value.to_token_stream())?;
                set(&mut self.description, written, meta)?;
            }
            Meta::NameValue(pair) if row && pair.path.is_ident("role") => {
                let role = match &pair.value {
                    Expr::Path(path) if path.path.get_ident().is_some() => path.path.get_ident().unwrap().to_string(),
                    other => text(other)?,
                };
                set(&mut self.role, role, meta)?;
            }
            Meta::NameValue(pair) if row && pair.path.is_ident("unit") => set(&mut self.unit, text(&pair.value)?, meta)?,
            Meta::List(list) if list.path.is_ident("tags") => {
                let tags = Punctuated::<syn::LitStr, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
                let mut unique = std::collections::BTreeSet::new();
                for tag in &tags {
                    if !unique.insert(tag.value()) {
                        return Err(Error::new(tag.span(), "a metadata tag is declared only once"));
                    }
                }
                set(&mut self.tags, tags.into_iter().map(|tag| tag.value()).collect(), meta)?;
            }
            Meta::List(list) if list.path.is_ident("attributes") => {
                let pairs = Punctuated::<Attribute, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
                set(&mut self.attributes, attributes(pairs)?, meta)?;
            }
            _ => return Ok(false),
        }

        Ok(true)
    }

    pub(crate) fn metadata(&self, identifier: &str, attrs: &[syn::Attribute]) -> Value {
        let description = crate::artifact::description(self.description.clone(), attrs);
        let description = (!description.is_empty()).then_some(description);
        json!({"identifier": identifier, "label": self.label.as_deref().unwrap_or(identifier),
            "description": description, "tags": self.tags.clone().unwrap_or_default(),
            "attributes": self.attributes.clone().unwrap_or_default()})
    }
}

/* === Public Functions === */

pub(crate) fn fields(fields: &mut Fields) -> Result<String, Error> {
    let mut rows = Vec::new();
    for field in fields {
        let mut presentation = Presentation::default();
        let mut retained = Vec::new();
        for attribute in std::mem::take(&mut field.attrs) {
            if !attribute.path().is_ident("row") {
                retained.push(attribute);
                continue;
            }
            for meta in attribute.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)? {
                if !presentation.parse(&meta, true)? {
                    return Err(Error::new(meta.span(), "row accepts description, label, tags, attributes, role, and unit"));
                }
            }
        }
        field.attrs = retained;
        let Some(name) = &field.ident else {
            continue;
        };
        let span = name.span();
        let name = name.to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name);
        if !name.is_ascii() {
            return Err(Error::new(span, "a row field requires an ASCII identifier for portable metadata"));
        }
        presentation.default_label(name);
        rows.push(json!({"metadata": presentation.metadata(name, &field.attrs), "role": presentation.role, "unit": presentation.unit}));
    }

    Ok(Value::Array(rows).to_string())
}

/* === Private Functions === */

fn key(input: ParseStream<'_>) -> syn::Result<(String, proc_macro2::Span)> {
    if input.peek(syn::LitStr) {
        let key: syn::LitStr = input.parse()?;
        return Ok((key.value(), key.span()));
    }
    let key: syn::Ident = input.parse().map_err(|_| input.error("a metadata key is an identifier or quoted string"))?;
    let name = key.to_string();

    Ok((name.strip_prefix("r#").unwrap_or(&name).to_owned(), key.span()))
}

fn attributes(pairs: impl IntoIterator<Item = Attribute>) -> syn::Result<serde_json::Map<String, Value>> {
    let mut attributes = serde_json::Map::new();
    for pair in pairs {
        if attributes.insert(pair.key.clone(), pair.value).is_some() {
            return Err(Error::new(pair.span, format!("attribute `{}` is declared twice", pair.key)));
        }
    }

    Ok(attributes)
}

fn title(name: &str) -> String {
    let chars: Vec<_> = name.chars().collect();
    let mut words = String::new();
    for (index, character) in chars.iter().copied().enumerate() {
        if character == '_' || character == '-' {
            words.push(' ');
            continue;
        }
        let previous = index.checked_sub(1).map(|index| chars[index]);
        let next = chars.get(index + 1);
        if character.is_uppercase()
            && previous.is_some_and(|previous| {
                previous.is_lowercase()
                    || previous.is_numeric()
                    || (previous.is_uppercase() && next.is_some_and(|next| next.is_lowercase()))
            })
        {
            words.push(' ');
        }
        words.push(character);
    }

    words
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map(|first| first.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn set<T>(slot: &mut Option<T>, value: T, meta: &Meta) -> Result<(), Error> {
    if slot.replace(value).is_some() {
        return Err(Error::new(meta.span(), "metadata property is declared twice"));
    }

    Ok(())
}

fn text(expr: &Expr) -> Result<String, Error> {
    syn::parse2::<syn::LitStr>(expr.to_token_stream()).map(|value| value.value())
}

fn literal(expr: &Expr) -> Result<Value, Error> {
    let (kind, value) = match expr {
        Expr::Lit(expr) => match &expr.lit {
            Lit::Str(value) => ("text", json!(value.value())),
            Lit::Bool(value) => ("bool", json!(value.value)),
            Lit::Int(value) => ("unsigned", json!(value.base10_parse::<u64>()?.to_string())),
            Lit::Float(value) => ("number", json!(value.base10_parse::<f64>()?.to_string())),
            _ => return Err(Error::new(expr.span(), "unsupported metadata literal")),
        },
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
            let Expr::Lit(inner) = unary.expr.as_ref() else {
                return Err(Error::new(expr.span(), "expected a numeric literal"));
            };
            match &inner.lit {
                Lit::Int(value) => (
                    "integer",
                    json!(
                        format!("-{}", value.base10_digits())
                            .parse::<i64>()
                            .map_err(|_| Error::new(value.span(), "signed integer is out of range"))?
                            .to_string()
                    ),
                ),
                Lit::Float(value) => ("number", json!((-value.base10_parse::<f64>()?).to_string())),
                _ => return Err(Error::new(expr.span(), "expected a numeric literal")),
            }
        }
        _ => {
            return Err(Error::new(expr.span(), "metadata attributes require literals"));
        }
    };

    Ok(json!({"type": kind, "value": value}))
}

/* === Tests === */
