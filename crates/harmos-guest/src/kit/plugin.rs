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
use harmos_sdk::operations::{Contracts, WireCommand, WireTable};
use harmos_sdk::plugin::{Declared, Owner, Plugin};

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
                ::std::vec::Vec<$crate::kit::plugin::__bindings::harmos::guest::capabilities::Registration>,
                ::std::string::String,
            > {
                $crate::kit::plugin::registrations::<$plugin>()
            }
            async fn operate(
                owner: ::std::string::String, command: ::std::vec::Vec<u8>,
            ) -> ::core::result::Result<::std::vec::Vec<u8>, $crate::kit::plugin::__bindings::harmos::guest::capabilities::Refusal> {
                let capacity = ::std::num::NonZeroUsize::new($capacity)
                    .ok_or_else(|| $crate::kit::plugin::refusal("plugin operation capacity must be positive"))?;
                $crate::kit::plugin::operate::<$plugin>(&owner, command, capacity).await
            }
        }
        $crate::kit::plugin::__bindings::export!(__HarmosPluginAdapter);
    };
}

/* === Type Aliases === */

type Table = Arc<Mutex<WireTable>>;
type Refusal = __bindings::harmos::guest::capabilities::Refusal;

/* === Structs === */

struct Instance {
    owner: TypeId,
    assigned_owner: String,
    capacity: NonZeroUsize,
    table: Table,
    assignments: harmos_sdk::operations::OperationAssignments,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn registrations<P: Plugin + Declared>() -> Result<Vec<__bindings::harmos::guest::capabilities::Registration>, String> {
    supported_observers::<P>()?;
    super::context::retain();
    std::hint::black_box(P::markers().map_err(|error| error.to_string())?);
    let mut registrar = harmos_domain::Registrar::new();
    <P as harmos_sdk::Plugin>::register(&mut registrar).map_err(|error| error.to_string())?;
    Ok(registrar
        .finish()
        .into_iter()
        .map(|entry| __bindings::harmos::guest::capabilities::Registration {
            namespace: entry.namespace,
            schema: entry.schema,
            payload: entry.payload,
        })
        .collect())
}

#[tracing::instrument(level = "debug", skip_all, fields(command_bytes = command.len()))]
pub async fn operate<P>(assigned_owner: &str, command: Vec<u8>, capacity: NonZeroUsize) -> Result<Vec<u8>, Refusal>
where
    P: Plugin<Context = Context> + Declared,
{
    supported_observers::<P>().map_err(refusal)?;
    super::context::retain();
    // Decode Before Allocating An Instance Or Preparing Authored State
    let command: WireCommand = WireCommand::decode(&command).map_err(refusal)?;
    let (table, assignments) = TABLE.with_borrow_mut(|active| {
        if let Some(instance) = active {
            if instance.owner != TypeId::of::<P>() || instance.capacity != capacity || instance.assigned_owner != assigned_owner {
                return Err(refusal("one component instance has one plugin and capacity"));
            }
            return Ok((Arc::clone(&instance.table), instance.assignments.clone()));
        }
        let markers = P::markers().map_err(refusal)?;
        std::hint::black_box(&markers);
        let required = harmos_domain::inventory::contracts::Required::from_tree(P::REQUIRES).map_err(refusal)?;
        let provides = Contracts::from_tree(P::PROVIDES).map_err(refusal)?;
        let listeners = harmos_domain::listeners::Declarations::from_tree(P::LISTENERS).map_err(refusal)?;
        harmos_domain::inventory::listeners::validate(&listeners, &provides).map_err(refusal)?;
        let declaration = harmos_domain::inventory::Declaration::inspect(&markers.concat(), harmos_domain::inventory::Facility::Guest)
            .map_err(refusal)?;
        harmos_domain::inventory::publications::validate_match(P::STREAMS, &declaration.streams).map_err(refusal)?;
        let contracts = &declaration.contracts;
        let sdk = [
            harmos_operations::lifecycle::Descriptor::of_initialize::<P>().map_err(refusal)?,
            harmos_operations::lifecycle::Descriptor::of_shutdown::<P>().map_err(refusal)?,
        ];
        if declaration.id != P::ID || contracts.provides != provides || contracts.requires != required || contracts.lifecycle != sdk {
            return Err(refusal("SDK manifest differs from authored contracts"));
        }
        if declaration.listeners != listeners {
            return Err(refusal("listener manifest differs from authored selection"));
        }
        if declaration.sections != P::CONTRIBUTIONS {
            return Err(refusal("contribution sections differ from authored selection"));
        }
        let (owner, table) = Owner::<P>::new(assigned_owner, provides, capacity).map_err(refusal)?;
        harmos_operations::lifecycle::bind(owner, &table).map_err(refusal)?;
        let assignments = harmos_sdk::operations::OperationAssignments::default();
        *active = Some(Instance {
            owner: TypeId::of::<P>(),
            assigned_owner: assigned_owner.to_owned(),
            capacity,
            table: Arc::clone(&table),
            assignments: assignments.clone(),
        });
        Ok((table, assignments))
    })?;
    let token = match &command {
        WireCommand::Initialize { context, .. } | WireCommand::Attempt { context, .. } | WireCommand::StreamAttempt { context, .. } => {
            Some(*context)
        }
        _ => None,
    };
    let values = match &command {
        WireCommand::Attempt { values, .. } | WireCommand::StreamAttempt { values, .. } => values.clone(),
        _ => Vec::new(),
    };
    let call = crate::operations::CallContext::from_portable(values).map_err(refusal)?;
    let context = Arc::new(Context {
        context: token,
        generation_context: None,
        assignments,
        call: Arc::new(call),
    });
    let call = context.context().clone();
    let mut dependencies = crate::Dependencies::new();
    dependencies.insert(Arc::new(harmos_sdk::operations::Calls(context.clone()))).map_err(refusal)?;
    dependencies.insert(context).map_err(refusal)?;
    let context = crate::OperationContext::new(dependencies, call);
    let phase = table.lock().map_err(refusal)?.begin(command, Arc::new(context));
    // The Original Slot Owns Work While Both Instance And Table Borrows Are Free
    let answer = phase.await;
    answer.encode().map_err(refusal)
}

/* === Private Functions === */

fn supported_observers<P: Declared>() -> Result<(), String> {
    if harmos_domain::catalog::count(P::OBSERVERS) != 0 {
        return Err("guest observer callbacks are unsupported".into());
    }
    Ok(())
}

pub fn refusal(error: impl std::fmt::Display) -> Refusal {
    let refusal = harmos_domain::Refusal::new(harmos_domain::UNREADABLE, error);
    Refusal {
        code: refusal.code().to_string(),
        failure: refusal.failure().encode(),
    }
}

/* === Tests === */
