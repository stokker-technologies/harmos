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
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Error, FnArg, GenericArgument, LitInt, LitStr, Meta, Path, PathArguments, Token, Type};
// Local Imports
use super::contribution::{asynchronous, block, borrowed, declared, parameter, required, returned};
use super::set_once;
use crate::path::{self, HarmosPath};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(crate) fn declare(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute, &item) {
        Ok(expanded) => expanded,
        Err(refused) => {
            let refusal = refused.to_compile_error();
            quote! { #item #refusal }
        }
    }
}

/* === Private Functions === */
fn expand(attribute: TokenStream, item: &TokenStream) -> Result<TokenStream, Error> {
    let (harmos, interval, streams, demand_keys, activation, description, id) = arguments(attribute)?;
    let block = block(item.clone(), "#[harmos::observer]")?;
    let name = &block.self_ty;
    let id = id.map(|id| quote!(#id)).unwrap_or_else(|| quote!(concat!(module_path!(), "::", stringify!(#name))));
    let description = super::description(description, &block.attrs);

    let observing =
        required(&block, "observe", "async fn observe(&mut self, context: &ObserverContext<'_, Resources>) -> Result<(), Error>")?;
    asynchronous(observing, "a observe awaits its instrument without blocking the executor")?;
    let (resources, host) = resources(observing)?;
    let host = host.map(|ty| quote!(#ty)).unwrap_or_else(|| quote!(#harmos::__private::sidecar::Supervisor));
    let observed = returned(observing)?;
    let construction = if let Some(opening) = declared(&block, "initialize") {
        asynchronous(opening, "initializing awaits whatever the observer acquires")?;
        if opening.sig.inputs.len() != 1 || !matches!(opening.sig.inputs.first(), Some(FnArg::Typed(_))) {
            return Err(Error::new(
                opening.sig.inputs.span(),
                "initialize takes the resource bundle and returns the observer: `async fn initialize(resources: &Resources) -> Result<Self, Error>`",
            ));
        }
        borrowed(&parameter(opening.sig.inputs.first().expect("one typed parameter"))?, "the bundle an observer binds")?;
        returned(opening)?;
        quote!(<Self>::initialize(resources))
    } else {
        quote! {{
            let _ = resources;
            #[diagnostic::on_unimplemented(message = "derive Default or provide async fn initialize(resources: &Resources) -> Result<Self, Error> for this observer")]
            trait ObserverDefault: ::core::default::Default {}
            impl<T: ::core::default::Default> ObserverDefault for T {}
            fn construct_default<T: ObserverDefault>() -> T { T::default() }

            ::core::future::ready(::core::result::Result::Ok(construct_default::<Self>()))
        }}
    };

    let answer = quote!(#harmos::__private::Answer);
    if let Type::Path(path) = &observed
        && let Some(segment) = path.path.segments.last()
        && segment.ident == "Result"
        && let PathArguments::AngleBracketed(arguments) = &segment.arguments
        && let Some(GenericArgument::Type(ok)) = arguments.args.first()
        && !matches!(ok, Type::Tuple(tuple) if tuple.elems.is_empty())
    {
        return Err(Error::new(
            ok.span(),
            "observe returns Result<(), Error>; publish zero or more values through context.publish_on::<Stream>(&row).await instead of returning a row",
        ));
    }
    let activation = if activation == "demand" { quote!(Demand) } else { quote!(Plugin) };

    let closing = ["stop", "cleanup"]
        .into_iter()
        .filter_map(|name| declared(&block, name))
        .map(|closing| {
            asynchronous(closing, "stopping awaits whatever the retained instance releases")?;
            if closing.sig.inputs.len() != 1
                || !matches!(closing.sig.inputs.first(), Some(FnArg::Receiver(receiver))
                    if matches!(receiver.kind, syn::ReceiverKind::Reference(_, _, Some(_))))
            {
                return Err(Error::new(
                    closing.sig.inputs.span(),
                    "observer cleanup borrows its original owner: use &mut self and return Result<(), Error>",
                ));
            }
            returned(closing)?;
            let name = &closing.sig.ident;
            Ok::<_, Error>(quote! {
                fn #name(&mut self) -> impl ::core::future::Future<Output = ::core::result::Result<(), Self::Error>> + Send {
                    <Self>::#name(self)
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote! {
        #item

        #[automatically_derived]
        impl #harmos::__private::PluginInventory::observers::Declared for #name {
            const ENTRY: #harmos::__private::PluginInventory::observers::Entry =
                #harmos::__private::PluginInventory::observers::Entry {
                    id: #id, description: #description,
                    interval: ::core::time::Duration::from_millis(#interval),
                    activation: #harmos::__private::sidecar::Activation::#activation,
                    streams: &[#(#harmos::__private::PluginInventory::publications::Entry::of::<#streams>()),*],
                    demand_keys: &[#(#demand_keys),*],
                };
        }

        #[automatically_derived]
        impl #harmos::__private::sidecar::Observer<#host> for #name {
            type Resources = #resources;
            type Error = <#observed as #answer>::Err;
            const ID: &'static str = <Self as #harmos::__private::PluginInventory::observers::Declared>::ENTRY.id;
            const DESCRIPTION: &'static str = <Self as #harmos::__private::PluginInventory::observers::Declared>::ENTRY.description;
            const INTERVAL: ::core::time::Duration = <Self as #harmos::__private::PluginInventory::observers::Declared>::ENTRY.interval;
            const STREAMS: &'static [#harmos::__private::sidecar::inventory::publications::Entry] = <Self as #harmos::__private::PluginInventory::observers::Declared>::ENTRY.streams;
            const DEMAND_KEYS: &'static [#harmos::__private::sidecar::DemandKey] = <Self as #harmos::__private::PluginInventory::observers::Declared>::ENTRY.demand_keys;
            const ACTIVATION: #harmos::__private::sidecar::Activation = <Self as #harmos::__private::PluginInventory::observers::Declared>::ENTRY.activation;

            // The author's own inherent functions are what run: an inherent
            // associated function shadows the trait's, so these delegations
            // reach the lines that were written rather than loop back here.
            fn initialize(
                resources: &Self::Resources,
            ) -> impl ::core::future::Future<
                Output = ::core::result::Result<Self, Self::Error>,
            > + Send {
                #construction
            }

            fn observe(
                &mut self,
                context: &#harmos::__private::sidecar::ObserverContext<'_, Self::Resources, #host>,
            ) -> impl ::core::future::Future<
                Output = ::core::result::Result<(), Self::Error>,
            > + Send {
                <Self>::observe(self, context)
            }

            #(#closing)*
        }
    })
}

fn resources(observing: &syn::ImplItemFn) -> Result<(Type, Option<Type>), Error> {
    let valid_receiver = matches!(observing.sig.inputs.first(), Some(FnArg::Receiver(receiver)) if matches!(receiver.kind, syn::ReceiverKind::Reference(_, _, Some(_))));
    if observing.sig.inputs.len() != 2 || !valid_receiver {
        return Err(Error::new(observing.sig.inputs.span(), "observe takes `&mut self` and `context: &ObserverContext<'_, Resources>`"));
    }
    let context = borrowed(&parameter(&observing.sig.inputs[1])?, "the observer context")?;
    let Type::Path(path) = &context else {
        return Err(Error::new(context.span(), "observe receives `&ObserverContext<'_, Resources>`"));
    };
    let segment = path.path.segments.last().expect("a type path has a segment");
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(Error::new(
            context.span(),
            "replace the old `&Tick` parameter with `&ObserverContext<'_, Resources>`; access timing with context.tick()",
        ));
    };
    let types: Vec<_> = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect();
    if segment.ident != "ObserverContext"
        || !(1..=2).contains(&types.len())
        || arguments.args.iter().any(|argument| !matches!(argument, GenericArgument::Type(_) | GenericArgument::Lifetime(_)))
    {
        return Err(Error::new(context.span(), "observe receives `&ObserverContext<'_, Resources>`"));
    }

    Ok((types[0].clone(), types.get(1).map(|ty| (*ty).clone())))
}

#[allow(clippy::type_complexity)]
fn arguments(attribute: TokenStream) -> Result<(HarmosPath, LitInt, Vec<Path>, Vec<Path>, String, Option<LitStr>, Option<LitStr>), Error> {
    let span = attribute.span();
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attribute)?;
    let mut harmos_path: Option<Path> = None;
    let mut interval = None;
    let mut streams = None;
    let mut demand_keys = None;
    let mut activation = None;
    let mut description = None;
    let mut id = None;

    for argument in &arguments {
        if let Some(parsed) = path::declared(argument) {
            set_once(&mut harmos_path, parsed?, argument.span(), path::KEY)?;
            continue;
        }
        match argument {
            Meta::NameValue(named) if named.path.is_ident("id") => {
                let value = syn::parse2::<LitStr>(named.value.to_token_stream())?;
                if value.value().trim().is_empty() || value.value().chars().any(char::is_control) {
                    return Err(Error::new(value.span(), "an observer identity must be nonempty and contain no control characters"));
                }
                set_once(&mut id, value, named.span(), "id")?;
            }
            Meta::List(listed) if listed.path.is_ident("streams") => set_once(
                &mut streams,
                listed.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?.into_iter().collect::<Vec<_>>(),
                listed.span(),
                "streams",
            )?,
            Meta::List(listed) if listed.path.is_ident("demand_keys") => set_once(
                &mut demand_keys,
                listed.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?.into_iter().collect::<Vec<_>>(),
                listed.span(),
                "demand_keys",
            )?,
            Meta::NameValue(named) if named.path.is_ident("activation") => {
                set_once(&mut activation, syn::parse2::<syn::LitStr>(named.value.to_token_stream())?, named.span(), "activation")?
            }
            Meta::NameValue(named) if named.path.is_ident("description") => {
                set_once(&mut description, syn::parse2::<LitStr>(named.value.to_token_stream())?, named.span(), "description")?
            }
            Meta::NameValue(named) if named.path.is_ident("stream") => {
                return Err(Error::new(
                    named.span(),
                    "replace `stream = Row` with `streams(Row)` and return Result<()>; publish through the observe context",
                ));
            }
            Meta::NameValue(named) if named.path.is_ident("interval_ms") => {
                set_once(&mut interval, syn::parse2::<LitInt>(named.value.to_token_stream())?, named.span(), "interval_ms")?
            }
            other => {
                return Err(Error::new(
                    other.span(),
                    "an observer declares `interval_ms = …`, with optional `streams(A, B)`, `demand_keys(KEY, OTHER)`, `activation = \"plugin\"` or `\"demand\"`, and `harmos_path = …`",
                ));
            }
        }
    }

    let Some(interval) = interval else {
        return Err(Error::new(
            span,
            "an observer declares `interval_ms = …`: cadence belongs to the observer, never to \
             whoever leased it",
        ));
    };
    if interval.base10_parse::<u64>()? == 0 {
        return Err(Error::new(interval.span(), "an observer's interval is at least 1 millisecond"));
    }

    let streams = streams.unwrap_or_default();
    let demand_keys = demand_keys.unwrap_or_default();
    let activation = activation.map(|value| value.value()).unwrap_or_else(|| "plugin".into());
    if !matches!(activation.as_str(), "plugin" | "demand") {
        return Err(Error::new(span, "activation is either `plugin` or `demand`"));
    }
    if activation == "demand" && streams.is_empty() && demand_keys.is_empty() {
        return Err(Error::new(span, "a demand observer requires `streams(A, B)` or `demand_keys(KEY, OTHER)`"));
    }

    Ok((HarmosPath::resolve(harmos_path), interval, streams, demand_keys, activation, description, id))
}

/* === Tests === */
