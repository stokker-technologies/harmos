/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod __bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "plugin",
        pub_export_macro: true,
        default_bindings_module: "$crate::kit::plugin::__bindings",
    });
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::any::TypeId;
use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
// Third Party Imports

// Local Imports
use super::context::Context;
use harmos_contract::operations::{Contracts, WireCommand, WireTable};
use harmos_contract::plugin::{Declared, Owner, Plugin};

/* === Constants === */

/* === Macros === */
thread_local! {
    static TABLE: RefCell<Option<Instance>> = const { RefCell::new(None) };
}

#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty, capacity = $capacity:expr) => {
        struct __HarmosPluginAdapter;
        impl $crate::kit::plugin::__bindings::Guest for __HarmosPluginAdapter {
            fn register() -> ::core::result::Result<
                ::std::vec::Vec<
                    $crate::kit::plugin::__bindings::harmos::guest::capabilities::Registration,
                >,
                ::std::string::String,
            > {
                $crate::kit::plugin::registrations::<$plugin>()
            }
            async fn operate(
                command: ::std::vec::Vec<u8>,
            ) -> ::core::result::Result<
                ::std::vec::Vec<u8>,
                $crate::kit::plugin::__bindings::harmos::guest::capabilities::Refusal,
            > {
                let capacity = ::std::num::NonZeroUsize::new($capacity).ok_or_else(|| {
                    $crate::kit::plugin::__bindings::harmos::guest::capabilities::Refusal {
                        code: "-32602".to_owned(),
                        message: "plugin operation capacity must be positive".to_owned(),
                    }
                })?;
                $crate::kit::plugin::operate::<$plugin>(command, capacity).await
            }
        }
        $crate::kit::plugin::__bindings::export!(__HarmosPluginAdapter);
    };
}

/* === Type Aliases === */
type Table = Arc<Mutex<WireTable<Context>>>;
type Refusal = __bindings::harmos::guest::capabilities::Refusal;

/* === Structs === */
struct Instance {
    owner: TypeId,
    capacity: NonZeroUsize,
    table: Table,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn registrations<P: Plugin + Declared>()
-> Result<Vec<__bindings::harmos::guest::capabilities::Registration>, String> {
    supported_observers::<P>()?;
    super::context::retain();
    std::hint::black_box(P::markers().map_err(|error| error.to_string())?);
    let mut registrar = harmos_contract::Registrar::new();
    <P as harmos_contract::Plugin>::register(&mut registrar).map_err(|error| error.to_string())?;
    Ok(registrar
        .finish()
        .into_iter()
        .map(
            |entry| __bindings::harmos::guest::capabilities::Registration {
                namespace: entry.namespace,
                schema: entry.schema,
                payload: entry.payload,
            },
        )
        .collect())
}

#[tracing::instrument(level = "debug", skip_all, fields(command_bytes = command.len()))]
pub async fn operate<P>(command: Vec<u8>, capacity: NonZeroUsize) -> Result<Vec<u8>, Refusal>
where
    P: Plugin<Context = Context> + Declared,
{
    supported_observers::<P>().map_err(refusal)?;
    super::context::retain();
    // Decode Before Allocating An Instance Or Preparing Authored State
    let command: WireCommand = WireCommand::decode(&command).map_err(refusal)?;
    let table = TABLE.with_borrow_mut(|active| {
        if let Some(instance) = active {
            if instance.owner != TypeId::of::<P>() || instance.capacity != capacity {
                return Err(refusal(
                    "one component instance has one plugin and capacity",
                ));
            }
            return Ok(Arc::clone(&instance.table));
        }
        std::hint::black_box(P::markers().map_err(refusal)?);
        let required = harmos_contract::inventory::contracts::Required::from_tree(P::REQUIRES)
            .map_err(refusal)?;
        let provides = Contracts::from_tree(P::PROVIDES).map_err(refusal)?;
        let listeners =
            harmos_contract::listeners::Declarations::from_tree(P::LISTENERS).map_err(refusal)?;
        harmos_contract::inventory::listeners::validate(&listeners, &provides).map_err(refusal)?;
        let declaration = harmos_contract::inventory::Declaration::inspect(
            &P::markers().map_err(refusal)?.concat(),
            harmos_contract::inventory::Facility::Guest,
        )
        .map_err(refusal)?;
        harmos_contract::inventory::publications::validate_match(P::STREAMS, &declaration.streams)
            .map_err(refusal)?;
        let contracts = &declaration.contracts;
        let sdk = [
            harmos_contract::operations::lifecycle::Descriptor::of_initialize::<P>()
                .map_err(refusal)?,
            harmos_contract::operations::lifecycle::Descriptor::of_shutdown::<P>()
                .map_err(refusal)?,
        ];
        if declaration.id != P::ID
            || contracts.provides != provides
            || contracts.requires != required
            || contracts.lifecycle != sdk
        {
            return Err(refusal("SDK manifest differs from authored contracts"));
        }
        if declaration.listeners != listeners {
            return Err(refusal("listener manifest differs from authored selection"));
        }
        if declaration.sections != P::CONTRIBUTIONS {
            return Err(refusal(
                "contribution sections differ from authored selection",
            ));
        }
        let (_, table) = Owner::<P>::new(provides, capacity).map_err(refusal)?;
        *active = Some(Instance {
            owner: TypeId::of::<P>(),
            capacity,
            table: Arc::clone(&table),
        });
        Ok(table)
    })?;
    let token = match &command {
        WireCommand::Initialize { context, .. }
        | WireCommand::Attempt { context, .. }
        | WireCommand::StreamAttempt { context, .. } => Some(*context),
        _ => None,
    };
    let values = match &command {
        WireCommand::Attempt { values, .. } | WireCommand::StreamAttempt { values, .. } => {
            values.clone()
        }
        _ => Vec::new(),
    };
    let call = crate::operations::CallContext::from_portable(values).map_err(refusal)?;
    let context = Arc::new(Context {
        context: token,
        generation_context: None,
        call: Arc::new(call),
    });
    let phase = table.lock().map_err(refusal)?.begin(command, context);
    // The Original Slot Owns Work While Both Instance And Table Borrows Are Free
    let answer = phase.await;
    answer.encode().map_err(refusal)
}

/* === Private Functions === */
fn supported_observers<P: Declared>() -> Result<(), String> {
    if harmos_contract::catalog::count(P::OBSERVERS) != 0 || !P::OBSERVER_CONTROLS.is_empty() {
        return Err("guest observer callbacks are unsupported".into());
    }
    Ok(())
}

fn refusal(error: impl std::fmt::Display) -> Refusal {
    Refusal {
        code: harmos_contract::UNREADABLE.to_string(),
        message: error.to_string(),
    }
}

/* === Tests === */
