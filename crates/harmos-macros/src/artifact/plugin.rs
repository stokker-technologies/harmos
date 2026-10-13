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
use super::set_once;
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Declaration {
    id: LitStr,
    context: Option<Type>,
    config: Option<Type>,
    restart: Option<super::restart::Policy>,
    description: Option<LitStr>,
    provides: Type,
    requires: Required,
    listeners: Type,
    observers: Type,
    observer_controls: Vec<Type>,
    contributions: Vec<Type>,
    streams: Vec<Path>,
    stream_entries: Option<Expr>,
    harmos: HarmosPath,
}

struct Listed {
    member: Type,
    grants: Vec<(Ident, LitStr)>,
}

/* === Enums === */
enum Required {
    Catalog(Box<Type>),
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
    let mut context = None;
    let mut config = None;
    let mut restart = None;
    let mut description = None;
    let mut provides = None;
    let mut requires = None;
    let mut listeners = None;
    let mut observers = None;
    let mut observer_controls = None;
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
            Meta::List(list) if list.path.is_ident("restart") => {
                set_once(
                    &mut restart,
                    super::restart::parse(&list)?,
                    list.span(),
                    "restart",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("restart") => {
                let value = syn::parse2::<LitStr>(named.value.to_token_stream())?;
                if value.value() != "never" {
                    return Err(Error::new(
                        value.span(),
                        "restart is an explicit policy or never",
                    ));
                }
                set_once(
                    &mut restart,
                    super::restart::Policy::NEVER,
                    named.span(),
                    "restart",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("context") => {
                set_once(
                    &mut context,
                    syn::parse2::<Type>(named.value.to_token_stream())?,
                    named.span(),
                    "context",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("config") => {
                set_once(
                    &mut config,
                    syn::parse2::<Type>(named.value.to_token_stream())?,
                    named.span(),
                    "config",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("id") => {
                set_once(
                    &mut id,
                    syn::parse2::<LitStr>(named.value.to_token_stream())?,
                    named.span(),
                    "id",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("description") => {
                set_once(
                    &mut description,
                    syn::parse2::<LitStr>(named.value.to_token_stream())?,
                    named.span(),
                    "description",
                )?;
            }
            Meta::NameValue(named)
                if named.path.is_ident("provides") || named.path.is_ident("requires") =>
            {
                let catalog = syn::parse2::<Type>(named.value.to_token_stream()).map_err(|_| Error::new(named.value.span(), "select a typed catalog or `()` with `provides = Catalog` and `requires = Catalog`"))?;
                if named.path.is_ident("provides") {
                    set_once(&mut provides, catalog, named.span(), "provides")?;
                } else {
                    set_once(
                        &mut requires,
                        Required::Catalog(Box::new(catalog)),
                        named.span(),
                        "requires",
                    )?;
                }
            }
            Meta::List(list) if list.path.is_ident("requires") => {
                let listed = list.parse_args_with(requirements)?;
                set_once(
                    &mut requires,
                    Required::Listed(listed),
                    list.span(),
                    "requires",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("listeners") => {
                let catalog = syn::parse2::<Type>(named.value.to_token_stream())?;
                set_once(&mut listeners, catalog, named.span(), "listeners")?;
            }
            Meta::NameValue(named) if named.path.is_ident("observers") => {
                set_once(
                    &mut observers,
                    syn::parse2::<Type>(named.value.to_token_stream())?,
                    named.span(),
                    "observers",
                )?;
            }
            Meta::List(list) if list.path.is_ident("observer_controls") => {
                let leaves =
                    list.parse_args_with(Punctuated::<Type, Token![,]>::parse_terminated)?;
                if leaves.len() != 2 {
                    return Err(Error::new(
                        list.span(),
                        "observer_controls requires actual Start and Stop wire operation types",
                    ));
                }
                set_once(
                    &mut observer_controls,
                    leaves.into_iter().collect::<Vec<_>>(),
                    list.span(),
                    "observer_controls",
                )?;
            }
            Meta::List(list) if list.path.is_ident("contributions") => {
                let listed =
                    list.parse_args_with(Punctuated::<Type, Token![,]>::parse_terminated)?;
                set_once(
                    &mut contributions,
                    listed.into_iter().collect::<Vec<_>>(),
                    list.span(),
                    "contributions",
                )?;
            }
            Meta::List(list) if list.path.is_ident("streams") => {
                let rows = list.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
                set_once(
                    &mut streams,
                    rows.into_iter().collect(),
                    list.span(),
                    "streams",
                )?;
            }
            Meta::NameValue(named) if named.path.is_ident("stream_entries") => {
                set_once(
                    &mut stream_entries,
                    named.value.clone(),
                    named.span(),
                    "stream_entries",
                )?;
            }
            other if other.path().is_ident("core") || other.path().is_ident("capabilities") => {
                return Err(Error::new(
                    other.span(),
                    "`core` and `capabilities` are retired; require host operations with grants: `requires(Op: survey(\"stream\"), Other)`",
                ));
            }
            other if other.path().is_ident("operations") => {
                return Err(Error::new(
                    other.span(),
                    "`operations` is retired for unified plugins; select explicit `provides` and `requires` catalogs",
                ));
            }
            other if other.path().is_ident("listeners") || other.path().is_ident("observers") => {
                return Err(Error::new(
                    other.span(),
                    "use typed `listeners = Audience` and `observers = Observers` selectors",
                ));
            }
            other if other.path().is_ident("version") => {
                return Err(super::version_argument(other.span()));
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "a plugin declares `id`, `provides`, `requires` (a catalog or a granted list), optional `description`, `contributions(Contribution, ...)`, typed `listeners` and `observers`, `observer_controls(Start, Stop)`, `streams(Row, ...)`, `stream_entries`, native `restart`, paired SDK `context` and `config`, and `harmos_path`",
                ));
            }
        }
    }
    let id = id.ok_or_else(|| Error::new(span, "a plugin requires `id = \"artifact-id\"`"))?;
    super::validate_id(&id)?;
    if context.is_some() != config.is_some() {
        return Err(Error::new(
            span,
            "SDK inert projection supplies both context and config types",
        ));
    }
    Ok(Declaration {
        id,
        context,
        config,
        restart,
        description,
        provides: provides.ok_or_else(|| {
            Error::new(
                span,
                "a plugin requires an explicit `provides = Catalog` or `provides = ()`",
            )
        })?,
        requires: requires.ok_or_else(|| {
            Error::new(
                span,
                "a plugin requires an explicit `requires = Catalog`, `requires(Op, ...)` or `requires = ()`",
            )
        })?,
        listeners: listeners.unwrap_or_else(|| syn::parse_quote!(())),
        observers: observers.unwrap_or_else(|| syn::parse_quote!(())),
        observer_controls: observer_controls.unwrap_or_default(),
        contributions: contributions.unwrap_or_default(),
        streams: streams.unwrap_or_default(),
        stream_entries,
        harmos: HarmosPath::resolve(harmos_path),
    })
}

fn requirements(input: ParseStream) -> Result<Vec<Listed>, Error> {
    let mut listed = Vec::new();
    while !input.is_empty() {
        let member = input.parse::<Type>()?;
        let mut grants = Vec::new();
        if input.peek(Token![:]) && !input.peek(Token![::]) {
            input.parse::<Token![:]>()?;
            loop {
                let kind = input.parse::<Ident>()?;
                if !matches!(kind.to_string().as_str(), "survey" | "egress" | "source") {
                    return Err(Error::new(
                        kind.span(),
                        "a requirement grants `survey(\"stream\")`, `egress(\"destination\")` or `source(\"tag\")`",
                    ));
                }
                let content;
                syn::parenthesized!(content in input);
                let selectors = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
                if selectors.is_empty() {
                    return Err(Error::new(
                        kind.span(),
                        "a grant names at least one selector",
                    ));
                }
                for selector in selectors {
                    if selector.value().is_empty() {
                        return Err(Error::new(selector.span(), "grant selectors are nonempty"));
                    }
                    grants.push((kind.clone(), selector));
                }
                if !grant_follows(input) {
                    break;
                }
                input.parse::<Token![,]>()?;
            }
        }
        listed.push(Listed { member, grants });
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(listed)
}

fn grant_follows(input: ParseStream) -> bool {
    let fork = input.fork();
    fork.parse::<Token![,]>().is_ok()
        && fork
            .parse::<Ident>()
            .is_ok_and(|kind| matches!(kind.to_string().as_str(), "survey" | "egress" | "source"))
        && fork.peek(syn::token::Paren)
}

fn implemented(block: &ItemImpl, projection: bool) -> Result<&Type, Error> {
    let Some((trait_path, _)) = &block.trait_ else {
        return Err(Error::new(
            block.span(),
            "`#[harmos::plugin]` belongs on `impl Plugin for ConcreteType`",
        ));
    };
    if block.modifiers.polarity.is_some()
        || (!projection
            && !trait_path.segments.last().is_some_and(|segment| {
                segment.ident == "Plugin" && matches!(segment.arguments, PathArguments::None)
            }))
    {
        return Err(Error::new(
            trait_path.span(),
            "`#[harmos::plugin]` requires the unified `Plugin` trait implementation",
        ));
    }
    let Type::Path(name) = block.self_ty.as_ref() else {
        return Err(Error::new(
            block.self_ty.span(),
            "a plugin implementation requires a concrete named type",
        ));
    };
    if !block.generics.params.is_empty()
        || block.generics.where_clause.is_some()
        || name.qself.is_some()
        || name
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return Err(Error::new(
            block.span(),
            "a plugin implementation is concrete and nongeneric",
        ));
    }
    Ok(block.self_ty.as_ref())
}

fn expand(attribute: TokenStream, item: &TokenStream) -> Result<TokenStream, Error> {
    let declared = declaration(attribute)?;
    let block: ItemImpl = syn::parse2(item.clone()).map_err(|_| {
        Error::new(
            item.span(),
            "`#[harmos::plugin]` belongs on `impl Plugin for ConcreteType`",
        )
    })?;
    let name = implemented(&block, declared.context.is_some())?.clone();
    let name = &name;
    let harmos = &declared.harmos;
    let mut block = block;
    let contributed = &declared.contributions;
    // A Product Projection Is Not `Plugin`; Its Owner Runs Contributions Itself.
    if !contributed.is_empty() && declared.context.is_none() {
        if block
            .items
            .iter()
            .any(|item| matches!(item, syn::ImplItem::Fn(method) if method.sig.ident == "register"))
        {
            return Err(Error::new(
                block.span(),
                "`contributions(...)` owns `Plugin::register`; register through a contribution instead",
            ));
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
    let description = super::description(declared.description, &block.attrs);
    let provides = &declared.provides;
    let requirement = quote!(#harmos::__private::PluginRequirement);
    let requires = match &declared.requires {
        Required::Catalog(catalog) => quote! {
            <#catalog as #harmos::catalog::InertCatalog<#harmos::__private::PluginRequirements, #requirement>>::ENTRIES
        },
        Required::Listed(listed) => {
            let leaves = listed.iter().map(|entry| {
                let member = &entry.member;
                let grants = entry.grants.iter().map(|(kind, selector)| {
                    let variant = format_ident!(
                        "{}",
                        match kind.to_string().as_str() {
                            "survey" => "Survey",
                            "egress" => "Egress",
                            _ => "Source",
                        }
                    );
                    quote!(#harmos::__private::PluginGrant::#variant(::std::borrow::Cow::Borrowed(#selector)))
                });
                quote! {
                    #harmos::catalog::Node::Leaf(#requirement::of::<
                        <#member as #harmos::operations::Declares>::Declaration
                    >(&[#(#grants),*]))
                }
            });
            quote!(&[#(#leaves),*])
        }
    };
    let rows = &declared.streams;
    let audience = &declared.listeners;
    let observed = &declared.observers;
    let controls = &declared.observer_controls;
    let projected = declared
        .stream_entries
        .as_ref()
        .map_or_else(|| quote!(&[]), |entries| quote!(#entries));
    let symbol = super::symbol(&name.to_token_stream().to_string().replace(' ', ""));
    let identity = format_ident!("__HARMOS_PLUGIN_IDENTITY_{symbol}");
    let contracts = format_ident!("__HARMOS_PLUGIN_CONTRACTS_{symbol}");
    let publications = format_ident!("__HARMOS_PLUGIN_PUBLICATIONS_{symbol}");
    let listeners = format_ident!("__HARMOS_PLUGIN_LISTENERS_{symbol}");
    let observers = format_ident!("__HARMOS_PLUGIN_OBSERVERS_{symbol}");
    let streams = format_ident!("__HARMOS_PLUGIN_STREAMS_{symbol}");
    let contribution_marker = format_ident!("__HARMOS_PLUGIN_CONTRIBUTIONS_{symbol}");
    let direct_entries = format_ident!("__HARMOS_PLUGIN_DIRECT_ENTRIES_{symbol}");
    let projected_entries = format_ident!("__HARMOS_PLUGIN_PROJECTED_ENTRIES_{symbol}");
    let context = declared.context.as_ref().map_or_else(
        || quote!(<#name as #harmos::Plugin>::Context),
        |context| quote!(#context),
    );
    let config = declared.config.as_ref().map_or_else(
        || quote!(<#name as #harmos::Plugin>::Config),
        |config| quote!(#config),
    );
    let facility = quote!(<#context as #harmos::__private::CarrierContext>::FACILITY);
    let inventory = quote!(#harmos::__private::PluginInventory);
    let declarations = quote!(#harmos::__private::PluginDeclared);
    let entry = quote!(#inventory::publications::Entry);
    let lifecycle = quote! {
        [
            <#name as #harmos::operations::lifecycle::Policy>::INITIALIZATION_POLICY,
            <#name as #harmos::operations::lifecycle::Policy>::SHUTDOWN_POLICY,
        ]
    };
    let restart_name = format_ident!("__HARMOS_PLUGIN_RESTART_{symbol}");
    let (restart_marker, restart_reference) = if let Some(policy) = declared.restart {
        let value = serde_json::json!({
            "kind": "restart", "facility": "sidecar", "owner": id.value(),
            "policy": { "credit": policy.credit, "sustained_health_ms": policy.sustained_health_ms,
                "backoff_base_ms": policy.backoff_base_ms, "backoff_cap_ms": policy.backoff_cap_ms }
        });
        let encoded = format!("\0HARMOS:ARTIFACT:V6:{value}\0").into_bytes();
        let length = encoded.len();
        let bytes = syn::LitByteStr::new(&encoded, id.span());
        (
            quote! {
                const _: () = assert!(#facility.as_bytes()[0] == b's', "resident restart policy requires native carrier");
                 #[used] static #restart_name: [u8; #length] = *#bytes;
            },
            quote!(, &#restart_name),
        )
    } else {
        (quote!(), quote!())
    };
    Ok(quote! {
        #block
        #restart_marker

        impl #declarations for #name {
            const ID: &'static str = #id;
            const VERSION: &'static str = ::core::env!("CARGO_PKG_VERSION");
            const DESCRIPTION: &'static str = #description;
            const PROVIDES: &'static [#harmos::catalog::Node<#harmos::operations::WireDeclaration>] =
                <#provides as #harmos::catalog::InertCatalog<#harmos::operations::Contracts, #harmos::operations::WireDeclaration>>::ENTRIES;
            const REQUIRES: &'static [#harmos::catalog::Node<#requirement>] = #requires;
            const LISTENERS: &'static [#harmos::catalog::Node<#harmos::listeners::Entry>] = <#audience as #harmos::catalog::InertCatalog<#harmos::listeners::Declarations, #harmos::listeners::Entry>>::ENTRIES;
            const OBSERVERS: &'static [#harmos::catalog::Node<#inventory::observers::Entry>] =
                <#observed as #harmos::catalog::InertCatalog<
                    #inventory::observers::Descriptors,
                    #inventory::observers::Entry
                >>::ENTRIES;
            const OBSERVER_CONTROLS: &'static [#harmos::operations::WireDeclaration] =
                &[#(#harmos::operations::WireDeclaration::of::<#controls>()),*];
            const STREAMS: &'static [#entry] = #streams;
            const CONTRIBUTIONS: &'static [#inventory::Section] = &[
                #(<#contributed as #harmos::__private::PluginContribution>::SECTION),*
            ];

            fn markers() -> ::std::result::Result<::std::vec::Vec<&'static [u8]>, #harmos::schema::graph::Error> {
                #harmos::__private::PluginSchemaMarkers::<Self>(
                    ::std::vec![#identity, #contracts, #publications, #listeners, #observers, #contribution_marker #restart_reference],
                    &#harmos::__private::SchemaSettings::default(),
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
            const LENGTH: usize = #inventory::contracts::encoded_tree_len(FACILITY, #id, PROVIDES, REQUIRES, &SDK);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::contracts::encode_tree(FACILITY, #id, PROVIDES, REQUIRES, &SDK);
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
            const CONTROLS: &[#harmos::operations::WireDeclaration] = <#name as #declarations>::OBSERVER_CONTROLS;
            #inventory::publications::require_tree_subset(ENTRIES, <#name as #declarations>::STREAMS);
            const _: () = assert!(
                (#harmos::catalog::count(ENTRIES) == 0 && CONTROLS.is_empty())
                    || (#harmos::catalog::count(ENTRIES) > 0 && CONTROLS.len() == 2),
                "nonempty observers require two actual observer_controls(Start, Stop) leaves"
            );
            const _: () = if FACILITY.as_bytes()[0] == b'g' {
                assert!(#harmos::catalog::count(ENTRIES) == 0, "guest observer callbacks are unsupported");
                assert!(CONTROLS.is_empty(), "guest observer controls are unsupported");
            };
            const LENGTH: usize = #inventory::observers::encoded_tree_len(FACILITY, #id, ENTRIES, CONTROLS);
            #[used]
            static MARKER: [u8; LENGTH] = #inventory::observers::encode_tree(FACILITY, #id, ENTRIES, CONTROLS);
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
