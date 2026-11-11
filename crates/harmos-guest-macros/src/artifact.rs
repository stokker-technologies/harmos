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
use syn::parse::{ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, Expr, Ident, ItemImpl, LitStr, Meta, Path, PathArguments, Token, Type};
// Local Imports
use crate::path::{self, HarmosPath};
use crate::set_once;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Declaration {
    id: LitStr,
    description: Option<LitStr>,
    requires: Required,
    listeners: Expr,
    observers: Type,
    contributions: Vec<Type>,
    streams: Vec<Path>,
    stream_entries: Option<Expr>,
    harmos: HarmosPath,
}

struct Listed {
    member: Expr,
    scopes: Vec<LitStr>,
}

/* === Enums === */

enum Required {
    Catalog(Box<Expr>),
    Listed(Vec<Listed>),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn declare(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, &item) {
        Ok(expanded) => expanded,
        Err(error) => {
            let refused = error.to_compile_error();
            quote!(#item #refused)
        }
    }
}

/* === Private Functions === */

fn declaration(attribute: TokenStream) -> Result<Declaration, Error> {
    let span = attribute.span();
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut id = None;
    let mut description = None;
    let mut requires = None;
    let mut listeners = None;
    let mut observers = None;
    let mut contributions = None;
    let mut streams = None;
    let mut stream_entries = None;
    let mut harmos_path = None;

    for argument in arguments {
        if let Some(parsed) = path::declared(&argument) {
            set_once(&mut harmos_path, parsed?, argument.span(), path::KEY)?;
            continue;
        }
        match argument {
            Meta::NameValue(named) if named.path.is_ident("id") => {
                set_once(&mut id, syn::parse2::<LitStr>(named.value.to_token_stream())?, named.span(), "id")?;
            }
            Meta::NameValue(named) if named.path.is_ident("description") => {
                set_once(&mut description, syn::parse2::<LitStr>(named.value.to_token_stream())?, named.span(), "description")?;
            }
            Meta::NameValue(named) if named.path.is_ident("requires") => {
                let catalog = named.value.clone();
                set_once(&mut requires, Required::Catalog(Box::new(catalog)), named.span(), "requires")?;
            }
            Meta::List(list) if list.path.is_ident("requires") => {
                let listed = list.parse_args_with(requirements)?;
                set_once(&mut requires, Required::Listed(listed), list.span(), "requires")?;
            }
            Meta::NameValue(named) if named.path.is_ident("listeners") => {
                set_once(&mut listeners, named.value.clone(), named.span(), "listeners")?;
            }
            Meta::NameValue(named) if named.path.is_ident("observers") => {
                set_once(&mut observers, syn::parse2::<Type>(named.value.to_token_stream())?, named.span(), "observers")?;
            }
            Meta::List(list) if list.path.is_ident("contributions") => {
                let listed = list.parse_args_with(Punctuated::<Type, Token![,]>::parse_terminated)?;
                set_once(&mut contributions, listed.into_iter().collect::<Vec<_>>(), list.span(), "contributions")?;
            }
            Meta::List(list) if list.path.is_ident("streams") => {
                let rows = list.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
                set_once(&mut streams, rows.into_iter().collect(), list.span(), "streams")?;
            }
            Meta::NameValue(named) if named.path.is_ident("stream_entries") => {
                set_once(&mut stream_entries, named.value.clone(), named.span(), "stream_entries")?;
            }
            other if other.path().is_ident("listeners") || other.path().is_ident("observers") => {
                return Err(Error::new(other.span(), "use `listeners = ENTRIES` and typed `observers = Observers` selectors"));
            }
            other if other.path().is_ident("version") => {
                return Err(crate::version_argument(other.span()));
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "a plugin declares `id`, `requires` (a catalog or a granted list), optional `description`, `contributions(Contribution, ...)`, typed `listeners` and `observers`, `streams(Row, ...)`, `stream_entries`, native `restart`, and `harmos_path`; provided catalogs belong in `Plugin::CATALOGS`",
                ));
            }
        }
    }
    let id = id.ok_or_else(|| Error::new(span, "a plugin requires `id = \"artifact_id\"`"))?;
    Ok(Declaration {
        id,
        description,
        requires: requires.unwrap_or_else(|| Required::Listed(Vec::new())),
        listeners: listeners.unwrap_or_else(|| syn::parse_quote!(&[])),
        observers: observers.unwrap_or_else(|| syn::parse_quote!(())),
        contributions: contributions.unwrap_or_default(),
        streams: streams.unwrap_or_default(),
        stream_entries,
        harmos: HarmosPath::resolve(harmos_path),
    })
}

fn requirements(input: ParseStream) -> Result<Vec<Listed>, Error> {
    let mut listed = Vec::new();
    while !input.is_empty() {
        let member = input.parse::<Expr>()?;
        let mut scopes = Vec::new();
        if input.peek(Token![:]) && !input.peek(Token![::]) {
            input.parse::<Token![:]>()?;
            let keyword = input.parse::<Ident>()?;
            if keyword != "scope" {
                return Err(Error::new(keyword.span(), "a requirement scopes its operation's grant with `scope(\"name\", ...)`"));
            }
            let content;
            syn::parenthesized!(content in input);
            let named = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
            if named.is_empty() {
                return Err(Error::new(keyword.span(), "a scope names at least one channel, stream or source"));
            }
            for scope in named {
                if scope.value().is_empty() {
                    return Err(Error::new(scope.span(), "grant scopes are nonempty"));
                }
                scopes.push(scope);
            }
        }
        listed.push(Listed { member, scopes });
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(listed)
}

fn implemented(block: &ItemImpl) -> Result<&Type, Error> {
    let Some((trait_path, _)) = &block.trait_ else {
        return Err(Error::new(block.span(), "`#[harmos::plugin]` belongs on `impl Plugin for ConcreteType`"));
    };
    if block.modifiers.polarity.is_some()
        || !trait_path.segments.last().is_some_and(|segment| segment.ident == "Plugin" && matches!(segment.arguments, PathArguments::None))
    {
        return Err(Error::new(trait_path.span(), "`#[harmos::plugin]` requires the unified `Plugin` trait implementation"));
    }
    let Type::Path(name) = block.self_ty.as_ref() else {
        return Err(Error::new(block.self_ty.span(), "a plugin implementation requires a concrete named type"));
    };
    if !block.generics.params.is_empty()
        || block.generics.where_clause.is_some()
        || name.qself.is_some()
        || name.path.segments.iter().any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return Err(Error::new(block.span(), "a plugin implementation is concrete and nongeneric"));
    }
    Ok(block.self_ty.as_ref())
}

fn expand(attribute: TokenStream, item: &TokenStream) -> Result<TokenStream, Error> {
    let declared = declaration(attribute)?;
    let block: ItemImpl =
        syn::parse2(item.clone()).map_err(|_| Error::new(item.span(), "`#[harmos::plugin]` belongs on `impl Plugin for ConcreteType`"))?;
    let name = implemented(&block)?.clone();
    let name = &name;
    let plugin = block.trait_.as_ref().expect("validated plugin trait").0.clone();
    let harmos = &declared.harmos;
    let mut block = block;
    let contributed = &declared.contributions;
    // A Product Projection Is Not `Plugin`; Its Owner Runs Contributions Itself.
    if !contributed.is_empty() {
        if block.items.iter().any(|item| matches!(item, syn::ImplItem::Fn(method) if method.sig.ident == "register")) {
            return Err(Error::new(block.span(), "`contributions(...)` owns `Plugin::register`; register through a contribution instead"));
        }
        let contribution = quote!(#harmos::__private::PluginContribution);
        block.items.push(syn::parse_quote! {
            fn register(
                registrar: &mut #harmos::__private::PluginRegistrar,
            ) -> ::core::result::Result<(), #harmos::__private::PluginRegistrationError> {
                #(<#contributed as #contribution>::register(registrar)?;)*
                ::core::result::Result::Ok(())
            }
        });
    }
    let id = &declared.id;
    let description = crate::description(declared.description, &block.attrs);
    let requirement = quote!(#harmos::__private::PluginRequirement);
    let requires = match &declared.requires {
        Required::Catalog(catalog) => {
            if matches!(catalog.as_ref(), Expr::Tuple(tuple) if tuple.elems.is_empty()) {
                quote!(&[])
            } else {
                quote! {
                    {
                        const DECLARATIONS: &[#harmos::catalog::Node<#harmos::operations::WireDeclaration>] = (#catalog).declarations;
                        &#harmos::__private::plugin::catalog_requirements::<{ #harmos::catalog::count(DECLARATIONS) }>(DECLARATIONS)
                    }
                }
            }
        }
        Required::Listed(listed) => {
            let leaves = listed.iter().map(|entry| {
                let member = &entry.member;
                let grants = entry.scopes.iter().map(|scope| {
                    quote!(#harmos::__private::PluginGrant {
                        operation: ::std::borrow::Cow::Borrowed(
                            (#member).entry().id,
                        ),
                        scope: ::core::option::Option::Some(::std::borrow::Cow::Borrowed(#scope)),
                    })
                });
                quote! {
                    #harmos::catalog::Node::Leaf(#requirement::new((#member).wire(), &[#(#grants),*]))
                }
            });
            quote!(&[#(#leaves),*])
        }
    };
    let rows = &declared.streams;
    let audience = &declared.listeners;
    let observed = &declared.observers;
    let projected = declared.stream_entries.as_ref().map_or_else(|| quote!(&[]), |entries| quote!(#entries));
    let symbol = crate::symbol(&name.to_token_stream().to_string().replace(' ', ""));
    let identity = format_ident!("__HARMOS_PLUGIN_IDENTITY_{symbol}");
    let contracts = format_ident!("__HARMOS_PLUGIN_CONTRACTS_{symbol}");
    let publications = format_ident!("__HARMOS_PLUGIN_PUBLICATIONS_{symbol}");
    let listeners = format_ident!("__HARMOS_PLUGIN_LISTENERS_{symbol}");
    let observers = format_ident!("__HARMOS_PLUGIN_OBSERVERS_{symbol}");
    let streams = format_ident!("__HARMOS_PLUGIN_STREAMS_{symbol}");
    let contribution_marker = format_ident!("__HARMOS_PLUGIN_CONTRIBUTIONS_{symbol}");
    let direct_entries = format_ident!("__HARMOS_PLUGIN_DIRECT_ENTRIES_{symbol}");
    let projected_entries = format_ident!("__HARMOS_PLUGIN_PROJECTED_ENTRIES_{symbol}");
    let context = quote!(<#name as #harmos::Lifecycle>::Context);
    let config = quote!(<#name as #harmos::Lifecycle>::Config);
    let facility = quote!(<#context as #harmos::__private::CarrierContext>::FACILITY);
    let inventory = quote!(#harmos::__private::PluginInventory);
    let declarations = quote!(#harmos::__private::PluginDeclared);
    let entry = quote!(#inventory::publications::Entry);
    let catalog = quote!(#harmos::operations::CatalogDescription);
    let declaration_node = quote!(#harmos::catalog::Node<#harmos::operations::WireDeclaration>);
    let callee_node = quote!(#harmos::catalog::Node<&'static [#harmos::operations::WireDeclaration]>);
    let catalog_provides = quote! {
        {
            const fn declarations<const N: usize>(catalogs: &'static [#catalog]) -> [#declaration_node; N] {
                let mut nodes = [const { #harmos::catalog::Node::Nested(&[]) }; N];
                let mut index = 0;
                while index < N {
                    let empty = ::core::mem::replace(
                        &mut nodes[index],
                        #harmos::catalog::Node::Nested(catalogs[index].declarations),
                    );
                    discard(empty);
                    index += 1;
                }
                nodes
            }
            const fn discard<T>(value: T) {
                ::core::mem::forget(value);
            }
            &declarations::<{ <#name as #plugin>::CATALOGS.len() }>(
                <#name as #plugin>::CATALOGS,
            )
        }
    };
    let catalog_callees = quote! {
        {
            const fn callees<const N: usize>(catalogs: &'static [#catalog]) -> [#callee_node; N] {
                let mut nodes = [const { #harmos::catalog::Node::Nested(&[]) }; N];
                let mut index = 0;
                while index < N {
                    let empty = ::core::mem::replace(
                        &mut nodes[index],
                        #harmos::catalog::Node::Nested(catalogs[index].callees),
                    );
                    discard(empty);
                    index += 1;
                }
                nodes
            }
            const fn discard<T>(value: T) {
                ::core::mem::forget(value);
            }
            &callees::<{ <#name as #plugin>::CATALOGS.len() }>(
                <#name as #plugin>::CATALOGS,
            )
        }
    };
    let lifecycle = quote! {
        [
            <#name as #harmos::model::operations::lifecycle::Policy>::INITIALIZATION_POLICY,
            <#name as #harmos::model::operations::lifecycle::Policy>::SHUTDOWN_POLICY,
        ]
    };
    Ok(quote! {
        #block

        const _: () = assert!(
            #harmos::__private::owner_segment(#id),
            "plugin id must match [a-z][a-z0-9_]* (one owner segment)"
        );

        impl #declarations for #name {
            const ID: &'static str = #id;
            const VERSION: &'static str = ::core::env!("CARGO_PKG_VERSION");
            const DESCRIPTION: &'static str = #description;
            const PROVIDES: &'static [#harmos::catalog::Node<#harmos::operations::WireDeclaration>] = #catalog_provides;
            const REQUIRES: &'static [#harmos::catalog::Node<#requirement>] = {
                const CALLEES: &[#harmos::catalog::Node<&'static [#harmos::operations::WireDeclaration]>] = #catalog_callees;
                const EXPLICIT: &[#harmos::catalog::Node<#requirement>] = #requires;
                const REQUIRED: [#harmos::catalog::Node<#requirement>; 1 + #harmos::__private::plugin::callee_count(CALLEES)] =
                    #harmos::__private::plugin::aggregate(EXPLICIT, <#name as #declarations>::PROVIDES, CALLEES);
                &REQUIRED
            };
            const LISTENERS: &'static [#harmos::catalog::Node<#harmos::listeners::Entry>] = #audience;
            const OBSERVERS: &'static [#harmos::catalog::Node<#inventory::observers::Entry>] =
                <#observed as #harmos::catalog::InertCatalog<
                    #inventory::observers::Descriptors,
                    #inventory::observers::Entry
                >>::ENTRIES;
            const STREAMS: &'static [#entry] = #streams;
            const CONTRIBUTIONS: &'static [#inventory::Section] = &[
                #(<#contributed as #harmos::__private::PluginContribution>::SECTION),*
            ];

            fn markers() -> ::std::result::Result<::std::vec::Vec<&'static [u8]>, #harmos::schema::graph::Error> {
                <Self as #declarations>::markers_with_settings(&#harmos::__private::SchemaSettings::default())
            }

            fn markers_with_settings(settings: &#harmos::__private::SchemaSettings)
                -> ::std::result::Result<::std::vec::Vec<&'static [u8]>, #harmos::schema::graph::Error>
            {
                #harmos::__private::PluginSchemaMarkers::<Self>(
                    ::std::vec![#identity, #contracts, #publications, #listeners, #observers, #contribution_marker ],
                    settings,
                )
            }
        }


        static #streams: &[#entry] = {
            const #direct_entries: &[#entry] = &[#(#entry::of::<#rows>()),*];
            const #projected_entries: &[#entry] = #projected;
            &#inventory::publications::merge::<{ #direct_entries.len() + #projected_entries.len() }>(#direct_entries, #projected_entries)
        };


        #[used]
        static #identity: &[u8] = {
            const FACILITY: &str = #facility;
            const VERSION: &str = <#name as #declarations>::VERSION;
            const CONFIG: &str = ::core::stringify!(#config);
            const LENGTH: usize = #inventory::contracts::identity_len(FACILITY, #id, VERSION, #description, CONFIG);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::contracts::encode_identity(FACILITY, #id, VERSION, #description, CONFIG);
            &MARKER
        };


        #[used]
        static #contracts: &[u8] = {
            const FACILITY: &str = #facility;
            const PROVIDES: &[#harmos::catalog::Node<#harmos::operations::WireDeclaration>] = <#name as #declarations>::PROVIDES;
            const REQUIRES: &[#harmos::catalog::Node<#requirement>] = <#name as #declarations>::REQUIRES;
            const SDK: [#harmos::operations::OperationPolicy; 2] = #lifecycle;
            const LENGTH: usize = #inventory::contracts::encoded_fragments_len(FACILITY, #id, PROVIDES, REQUIRES, &SDK);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::contracts::encode_fragments(FACILITY, #id, PROVIDES, REQUIRES, &SDK);
            &MARKER
        };


        #[used]
        static #listeners: &[u8] = {
            const FACILITY: &str = #facility;
            const ENTRIES: &[#harmos::catalog::Node<#harmos::listeners::Entry>] = <#name as #declarations>::LISTENERS;
            const LENGTH: usize = #inventory::listeners::encoded_len(FACILITY, #id, ENTRIES);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::listeners::encode(FACILITY, #id, ENTRIES);
            &MARKER
        };


        #[used]
        static #observers: &[u8] = {
            const FACILITY: &str = #facility;
            const ENTRIES: &[#harmos::catalog::Node<#inventory::observers::Entry>] = <#name as #declarations>::OBSERVERS;
            #inventory::publications::require_tree_subset(ENTRIES, <#name as #declarations>::STREAMS);
            const _: () = if FACILITY.as_bytes()[0] == b'g' {
                assert!(#harmos::catalog::count(ENTRIES) == 0, "guest observer callbacks are unsupported");
            };
            const LENGTH: usize = #inventory::observers::encoded_tree_len(FACILITY, #id, ENTRIES);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::observers::encode_tree(FACILITY, #id, ENTRIES);
            &MARKER
        };

        const _: () = assert!(
            #inventory::contributions::namespaced(
                <#name as #declarations>::CONTRIBUTIONS,
                &[#(<#contributed as #harmos::__private::PluginContribution>::NAMESPACE),*],
            ),
            "each contribution's section carries its own namespace"
        );


        #[used]
        static #contribution_marker: &[u8] = {
            const FACILITY: &str = #facility;
            const SECTIONS: &[#inventory::Section] = <#name as #declarations>::CONTRIBUTIONS;
            const LENGTH: usize = #inventory::contributions::encoded_len(FACILITY, #id, SECTIONS);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::contributions::encode(FACILITY, #id, SECTIONS);
            &MARKER
        };


        #[used]
        static #publications: &[u8] = {
            const FACILITY: &str = #facility;
            const ENTRIES: &[#entry] = <#name as #declarations>::STREAMS;
            const LENGTH: usize = #inventory::publications::encoded_len(FACILITY, #id, ENTRIES);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::publications::encode(FACILITY, #id, ENTRIES);
            &MARKER
        };
    })
}

/* === Tests === */
