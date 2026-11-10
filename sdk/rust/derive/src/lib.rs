/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type, parse_macro_input};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[proc_macro_derive(Message, attributes(harmos))]
pub fn message(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput)).unwrap_or_else(syn::Error::into_compile_error).into()
}

/* === Private Functions === */

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(&input, "Message Cannot Have Generic Parameters"));
    }
    let mut package = "harmos.plugin.v1".to_owned();
    let mut file = None;
    let mut protobuf_name = name.to_string();
    for attr in &input.attrs {
        if attr.path().is_ident("harmos") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("package") {
                    package = meta.value()?.parse::<syn::LitStr>()?.value();
                } else if meta.path.is_ident("name") {
                    protobuf_name = meta.value()?.parse::<syn::LitStr>()?.value();
                } else if meta.path.is_ident("file") {
                    file = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else {
                    return Err(meta.error("Expected package or file"));
                }
                Ok(())
            })?;
        }
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(&input, "Message Requires a Struct"));
    };
    let fields = match &data.fields {
        Fields::Named(fields) => fields.named.iter().collect::<Vec<_>>(),
        Fields::Unit => Vec::new(),
        _ => return Err(syn::Error::new_spanned(&input, "Message Requires Named Fields")),
    };
    let file = file.unwrap_or_else(|| format!("{}/{}.proto", package.replace('.', "/"), name));
    let full_name = format!("{package}.{protobuf_name}");
    let message_name = protobuf_name;
    let mut encodes = Vec::new();
    let mut merges = Vec::new();
    let mut lengths = Vec::new();
    let mut descriptors = Vec::new();
    let mut defaults = Vec::new();
    let mut dependencies = Vec::new();
    let mut tags = std::collections::BTreeSet::new();
    let mut oneofs = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let ident = field.ident.as_ref().unwrap();
        let field_name = ident.to_string().trim_start_matches("r#").to_owned();
        let mut tag = index as u32 + 1;
        for attr in &field.attrs {
            if attr.path().is_ident("harmos") {
                attr.parse_nested_meta(|meta| {
                    if !meta.path.is_ident("tag") {
                        return Err(meta.error("Expected tag"));
                    }
                    tag = meta.value()?.parse::<syn::LitInt>()?.base10_parse()?;
                    Ok(())
                })?;
            }
        }
        if tag == 0 || tag > 536_870_911 || (19_000..=19_999).contains(&tag) || !tags.insert(tag) {
            return Err(syn::Error::new_spanned(field, "Invalid or Duplicate Protobuf Tag"));
        }
        let mut ty = &field.ty;
        let optional = inner(ty, "Option").is_some();
        if optional {
            ty = inner(ty, "Option").unwrap();
        }
        let bytes = inner(ty, "Vec").is_some_and(|ty| type_name(ty).as_deref() == Some("u8"));
        let repeated = !bytes && inner(ty, "Vec").is_some();
        if repeated {
            ty = inner(ty, "Vec").unwrap();
        }
        if optional && repeated {
            return Err(syn::Error::new_spanned(field, "Optional Repeated Fields Are Not Protobuf"));
        }
        let scalar = match type_name(ty).as_deref() {
            Some("f64") => Some(("double", 1)),
            Some("f32") => Some(("float", 2)),
            Some("i64") => Some(("int64", 3)),
            Some("u64") => Some(("uint64", 4)),
            Some("i32") => Some(("int32", 5)),
            Some("bool") => Some(("bool", 8)),
            Some("String") => Some(("string", 9)),
            Some("u32") => Some(("uint32", 13)),
            Some("Vec") if bytes => Some(("bytes", 12)),
            _ => None,
        };
        let (module, kind) = scalar.unwrap_or(("message", 11));
        let module = syn::Ident::new(module, ident.span());
        let encoding = quote!(::harmos_sdk_grpc::encoding::#module);
        let type_name = if scalar.is_none() {
            dependencies.push(quote! {
                <#ty as ::harmos_sdk_grpc::Message>::register(types);
                let type_name = <#ty as ::harmos_sdk_grpc::Message>::name();
                let dependency = types.file.iter().find(|file| {
                    file.message_type.iter().any(|message| format!("{}.{}", file.package.as_deref().unwrap_or(""), message.name.as_deref().unwrap_or("")) == type_name)
                }).and_then(|file| file.name.clone());
                if let Some(dependency) = dependency.filter(|dependency| dependency != #file) {
                    let current = types.file.iter_mut().find(|file| file.name.as_deref() == Some(#file)).expect("Registered Message File");
                    if !current.dependency.contains(&dependency) { current.dependency.push(dependency); }
                }
            });
            quote!(Some(format!(".{}", <#ty as ::harmos_sdk_grpc::Message>::name())))
        } else {
            quote!(None)
        };
        let (encode, merge, length) = if repeated {
            (
                quote!(#encoding::encode_repeated(#tag, &self.#ident, buf);),
                quote!(#encoding::merge_repeated(wire_type, &mut self.#ident, buf, ctx)),
                quote!(#encoding::encoded_len_repeated(#tag, &self.#ident)),
            )
        } else if optional {
            (
                quote!(if let Some(value) = &self.#ident { #encoding::encode(#tag, value, buf); }),
                quote!(#encoding::merge(wire_type, self.#ident.get_or_insert_with(Default::default), buf, ctx)),
                quote!(self.#ident.as_ref().map_or(0, |value| #encoding::encoded_len(#tag, value))),
            )
        } else if scalar.is_none() {
            (
                quote!(#encoding::encode(#tag, &self.#ident, buf);),
                quote!(#encoding::merge(wire_type, &mut self.#ident, buf, ctx)),
                quote!(#encoding::encoded_len(#tag, &self.#ident)),
            )
        } else {
            let default = if bytes || kind == 9 { quote!(!self.#ident.is_empty()) } else { quote!(self.#ident != Default::default()) };
            (
                quote!(if #default { #encoding::encode(#tag, &self.#ident, buf); }),
                quote!(#encoding::merge(wire_type, &mut self.#ident, buf, ctx)),
                quote!(if #default { #encoding::encoded_len(#tag, &self.#ident) } else { 0 }),
            )
        };
        let label = if repeated { 3 } else { 1 };
        let (proto3_optional, oneof_index) = if optional && scalar.is_some() {
            let index = oneofs.len() as i32;
            let oneof_name = format!("_{field_name}");
            oneofs.push(quote!(::harmos_sdk_grpc::encoding::OneofDescriptorProto { name: Some(#oneof_name.into()), ..Default::default() }));
            (quote!(Some(true)), quote!(Some(#index)))
        } else {
            (quote!(None), quote!(None))
        };
        descriptors.push((
            tag,
            quote!(::harmos_sdk_grpc::encoding::FieldDescriptorProto {
                name: Some(#field_name.into()), number: Some(#tag as i32), label: Some(#label), r#type: Some(#kind),
                type_name: #type_name, proto3_optional: #proto3_optional, oneof_index: #oneof_index, ..Default::default()
            }),
        ));
        encodes.push((tag, encode));
        merges.push(quote!(#tag => #merge,));
        lengths.push(length);
        defaults.push(quote!(#ident: Default::default()));
    }
    encodes.sort_by_key(|(tag, _)| *tag);
    descriptors.sort_by_key(|(tag, _)| *tag);
    let encodes = encodes.iter().map(|(_, code)| code);
    let descriptors = descriptors.iter().map(|(_, code)| code);
    Ok(quote! {
        impl Default for #name { fn default() -> Self { Self { #(#defaults,)* } } }
        impl ::harmos_sdk_grpc::encoding::Message for #name {
            fn encode_raw(&self, buf: &mut impl ::harmos_sdk_grpc::encoding::BufMut) { #(#encodes)* }
            fn merge_field(&mut self, tag: u32, wire_type: ::harmos_sdk_grpc::encoding::WireType, buf: &mut impl ::harmos_sdk_grpc::encoding::Buf,
                ctx: ::harmos_sdk_grpc::encoding::DecodeContext) -> ::core::result::Result<(), ::harmos_sdk_grpc::encoding::DecodeError> {
                match tag { #(#merges)* _ => ::harmos_sdk_grpc::encoding::skip_field(wire_type, tag, buf, ctx) }
            }
            fn encoded_len(&self) -> usize { 0 #(+ #lengths)* }
            fn clear(&mut self) { *self = Default::default(); }
        }
        impl ::harmos_sdk_grpc::Message for #name {
            fn name() -> String { #full_name.into() }
            fn types() -> ::harmos_sdk_grpc::encoding::FileDescriptorSet {
                let mut types = ::harmos_sdk_grpc::encoding::FileDescriptorSet::default();
                Self::register(&mut types);
                types
            }
            fn register(types: &mut ::harmos_sdk_grpc::encoding::FileDescriptorSet) {
                let registered = types.file.iter().any(|file| file.package.as_deref() == Some(#package)
                    && file.message_type.iter().any(|message| message.name.as_deref() == Some(#message_name)));
                let own = ::harmos_sdk_grpc::encoding::FileDescriptorSet { file: vec![::harmos_sdk_grpc::encoding::FileDescriptorProto {
                    name: Some(#file.into()), package: Some(#package.into()), syntax: Some("proto3".into()),
                    message_type: vec![::harmos_sdk_grpc::encoding::DescriptorProto {
                        name: Some(#message_name.into()), field: vec![#(#descriptors,)*], oneof_decl: vec![#(#oneofs,)*], ..Default::default()
                    }], ..Default::default()
                }] };
                ::harmos_sdk_grpc::encoding::merge_descriptor_types(types, own);
                if registered { return; }
                #(#dependencies)*
            }
        }
    })
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path.segments.last().map(|segment| segment.ident.to_string())
}

fn inner<'a>(ty: &'a Type, container: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != container {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match arguments.args.first()? {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    }
}

/* === Tests === */
