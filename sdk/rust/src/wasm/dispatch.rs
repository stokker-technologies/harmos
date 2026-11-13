/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest Lifecycle Dispatch

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{
    Registration,
    context::{Import, Transport},
};
use crate::{Context, Error, Result, operation::LocalFuture};
use harmos_protocol::{
    v1 as host,
    v1::{self, guest_command, response},
};
use prost::Message as _;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::{Arc, OnceLock},
};
use tokio::sync::watch;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

const LIMIT: usize = 16 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// The component adapter; the import function carries bytes across the WIT boundary.
#[doc(hidden)]
#[derive(Clone)]
pub struct Adapter(Rc<Session>);
struct Session {
    registration: Registration,
    metadata: v1::GuestRegistration,
    import: Arc<Import>,
    owner: RefCell<Option<String>>,
    generations: RefCell<BTreeMap<String, u64>>,
    generation_context: RefCell<Vec<u8>>,
    assignments: Arc<OnceLock<host::OperationAssignments>>,
    slots: RefCell<BTreeMap<u64, Rc<Slot>>>,
    initialized: Cell<bool>,
    closed: Cell<bool>,
}
struct Slot {
    prepare: v1::GuestPrepare,
    context: RefCell<Option<Context>>,
    running: watch::Sender<bool>,
    cancellation: watch::Sender<Option<String>>,
    attempt: Cell<u32>,
    failed_initialization: Cell<bool>,
}
struct Loan(Rc<Slot>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Adapter {
    pub fn new(
        registration: Registration, import: impl Fn(Vec<u8>) -> LocalFuture<'static, Result<Vec<u8>>> + Send + Sync + 'static,
    ) -> Result<Self> {
        let metadata = registration.metadata()?;
        Ok(Self(Rc::new(Session {
            registration,
            metadata,
            import: Arc::new(import),
            owner: RefCell::new(None),
            generations: RefCell::new(BTreeMap::new()),
            generation_context: RefCell::new(Vec::new()),
            assignments: Arc::new(OnceLock::new()),
            slots: RefCell::new(BTreeMap::new()),
            initialized: Cell::new(false),
            closed: Cell::new(false),
        })))
    }
    pub fn metadata(&self) -> &v1::GuestRegistration {
        &self.0.metadata
    }
    pub async fn operate(&self, owner: &str, command: &[u8]) -> Vec<u8> {
        let result = self.0.operate(owner, command).await;
        let result = match result {
            Ok(payload) => response::Result::Payload(payload),
            Err(error) => match error.response("", &self.0.registration.plugin.id) {
                Ok(result) => result,
                Err(status) => response::Result::Failure(v1::Failure {
                    classification: v1::Classification::Refused as i32,
                    code: "invalid_guest_command".into(),
                    message: status.to_string(),
                    ..Default::default()
                }),
            },
        };
        v1::Response { result: Some(result) }.encode_to_vec()
    }
}
impl Session {
    async fn operate(&self, owner: &str, bytes: &[u8]) -> Result<Vec<u8>> {
        if bytes.len() > LIMIT {
            return Err(invalid("Guest Command Exceeds Its Byte Bound"));
        }
        let command = harmos_protocol::guest::decode_command(bytes).map_err(|error| invalid(error.to_string()))?;
        {
            let mut assigned = self.owner.borrow_mut();
            if assigned.as_ref().is_some_and(|assigned| assigned != owner) {
                return Err(invalid("Component Owner Changed"));
            }
            if owner.is_empty() {
                return Err(invalid("Component Owner Is Required"));
            }
            assigned.get_or_insert_with(|| owner.into());
        }
        let phase = command.phase.ok_or_else(|| invalid("Guest Phase Is Required"))?;
        match phase {
            guest_command::Phase::Prepare(prepare) => self.prepare(prepare),
            guest_command::Phase::Initialize(invocation) => {
                let slot = self.slot(invocation.invocation)?;
                if *slot.running.borrow() {
                    return Err(invalid("Guest Invocation Is Running"));
                }
                let context = self.context(&slot, invocation)?;
                *slot.context.borrow_mut() = Some(context);
                Ok(Vec::new())
            }
            guest_command::Phase::Attempt(invocation) => {
                let slot = self.slot(invocation.invocation)?;
                if slot.context.borrow().is_none() {
                    return Err(invalid("Guest Invocation Was Not Initialized"));
                }
                if *slot.running.borrow() {
                    return Err(invalid("Guest Invocation Is Already Running"));
                }
                let context = self.context(&slot, invocation)?;
                slot.attempt.set(context.attempt);
                slot.running.send_replace(true);
                let _loan = Loan(Rc::clone(&slot));
                let operation = self.local(&slot.prepare.operation)?;
                if operation == "harmos.initialize" {
                    if self.initialized.get() {
                        return Err(invalid("Guest Is Already Initialized"));
                    }
                    let initialize =
                        v1::GuestInitialize::decode(slot.prepare.payload.as_slice()).map_err(|error| invalid(error.to_string()))?;
                    if initialize.generation_context.len() != 32 {
                        return Err(invalid("Generation Capability Must Contain 32 Bytes"));
                    }
                    let assignments = initialize.assignments.ok_or_else(|| invalid("Guest Assignments Are Required"))?;
                    validate_assignments(&assignments)?;
                    self.assignments.set(assignments).map_err(|_| invalid("Guest Assignments Are Frozen"))?;
                    *self.generation_context.borrow_mut() = initialize.generation_context;
                    let mut context = context;
                    context.child = Some(self.transport(context.guest_transport()?.context.clone(), true));
                    slot.failed_initialization.set(true);
                    if let Some(initialize_callback) = &self.registration.initialize {
                        initialize_callback.run(&context, initialize.configuration).await?;
                    }
                    slot.failed_initialization.set(false);
                    self.initialized.set(true);
                    return Ok(Vec::new());
                }
                if operation == "harmos.shutdown" {
                    self.closed.set(true);
                    if let Some(shutdown) = &self.registration.shutdown {
                        shutdown.run(&context).await?;
                    }
                    self.initialized.set(false);
                    return Ok(Vec::new());
                }
                if !self.initialized.get() || self.closed.get() {
                    return Err(invalid("Guest Is Not Active"));
                }
                let handler = self
                    .registration
                    .plugin
                    .operations
                    .iter()
                    .find(|entry| entry.contract.id == operation)
                    .ok_or_else(|| invalid("Unknown Guest Operation"))?;
                handler.handler.execute(&context, slot.prepare.payload.clone()).await
            }
            guest_command::Phase::Cancel(invocation) => {
                if let Ok(slot) = self.slot(invocation) {
                    slot.cancellation.send_replace(Some("Host Cancelled".into()));
                }
                Ok(Vec::new())
            }
            guest_command::Phase::Finalize(invocation) => {
                let slot = self.slots.borrow().get(&invocation).cloned();
                let Some(slot) = slot else {
                    return Ok(Vec::new());
                };
                let mut running = slot.running.subscribe();
                while *running.borrow_and_update() {
                    running.changed().await.map_err(|error| invalid(error.to_string()))?;
                }
                let operation = self.local(&slot.prepare.operation)?;
                if operation == "harmos.shutdown" || slot.failed_initialization.get() {
                    for cleanup in self.registration.plugin.cleanup.iter().chain(&self.registration.cleanup) {
                        cleanup()?;
                    }
                }
                self.slots.borrow_mut().remove(&invocation);
                Ok(Vec::new())
            }
        }
    }
    fn local<'a>(&self, operation: &'a str) -> Result<&'a str> {
        let owner = self.owner.borrow();
        let owner = owner.as_ref().ok_or_else(|| invalid("Component Owner Is Required"))?;
        operation.strip_prefix(owner).and_then(|local| local.strip_prefix('.')).ok_or_else(|| invalid("Operation Owner Differs"))
    }
    fn prepare(&self, prepare: v1::GuestPrepare) -> Result<Vec<u8>> {
        if prepare.invocation == 0 || prepare.generation == 0 || prepare.fingerprint.len() != 32 {
            return Err(invalid("Invalid Guest Invocation Identity"));
        }
        if self.generations.borrow().get(&prepare.operation).is_some_and(|generation| *generation != prepare.generation) {
            return Err(invalid("Guest Generation Changed"));
        }
        let operation = self.local(&prepare.operation)?;
        let lifecycle = operation == "harmos.initialize" || operation == "harmos.shutdown";
        if !lifecycle {
            if !self.initialized.get() || self.closed.get() {
                return Err(invalid("Guest Is Not Active"));
            }
            let catalog = self.metadata.catalog.as_ref().expect("validated guest catalog");
            let contract =
                catalog.operations.iter().find(|entry| entry.id == operation).ok_or_else(|| invalid("Unknown Guest Operation"))?;
            if contract.fingerprint != prepare.fingerprint {
                return Err(invalid("Guest Operation Fingerprint Differs"));
            }
            let handler = self
                .registration
                .plugin
                .operations
                .iter()
                .find(|entry| entry.contract.id == operation)
                .expect("catalog and handlers share one builder");
            handler.handler.validate(&prepare.payload)?;
            let occupied = self
                .slots
                .borrow()
                .values()
                .filter(|slot| {
                    !slot.prepare.operation.ends_with(".harmos.initialize") && !slot.prepare.operation.ends_with(".harmos.shutdown")
                })
                .count();
            if occupied >= self.registration.capacity {
                return Err(invalid("Guest Operation Capacity Exhausted"));
            }
        } else if super::lifecycle_fingerprint(operation).map_err(|error| invalid(error.to_string()))? != prepare.fingerprint {
            return Err(invalid("Guest Lifecycle Fingerprint Differs"));
        }
        let mut slots = self.slots.borrow_mut();
        if slots.contains_key(&prepare.invocation) {
            return Err(invalid("Guest Invocation Already Exists"));
        }
        self.generations.borrow_mut().insert(prepare.operation.clone(), prepare.generation);
        slots.insert(
            prepare.invocation,
            Rc::new(Slot {
                prepare,
                context: RefCell::new(None),
                running: watch::channel(false).0,
                cancellation: watch::channel(None).0,
                attempt: Cell::new(0),
                failed_initialization: Cell::new(false),
            }),
        );
        Ok(Vec::new())
    }
    fn slot(&self, invocation: u64) -> Result<Rc<Slot>> {
        self.slots.borrow().get(&invocation).cloned().ok_or_else(|| invalid("Guest Invocation Was Not Prepared"))
    }
    fn transport(&self, context: Vec<u8>, retained: bool) -> Arc<dyn crate::context::ChildTransport> {
        Arc::new(Transport {
            import: Arc::clone(&self.import),
            context,
            generation_context: if retained { self.generation_context.borrow().clone() } else { Vec::new() },
            assignments: Arc::clone(&self.assignments),
        })
    }
    fn context(&self, slot: &Slot, invocation: v1::GuestInvocation) -> Result<Context> {
        if invocation.context.len() != 32 {
            return Err(invalid("Attempt Capability Must Contain 32 Bytes"));
        }
        harmos_protocol::guest::validate_values(&invocation.values).map_err(|error| invalid(error.to_string()))?;
        let attempt = slot.attempt.get().checked_add(1).ok_or_else(|| invalid("Guest Attempt Counter Exhausted"))?;
        let catalog = self.metadata.catalog.as_ref().expect("validated guest catalog");
        let mut context = Context::new(
            v1::Request {
                operation: slot.prepare.operation.clone(),
                invocation: slot.prepare.invocation.to_string(),
                attempt,
                ..Default::default()
            },
            self.registration.plugin.id.clone(),
            crate::Plugin::requirements(catalog),
        );
        context.local = Arc::new(
            self.registration
                .plugin
                .operations
                .iter()
                .filter(|operation| operation.contract.visibility == v1::Visibility::Private as i32)
                .map(|operation| (operation.contract.id.clone(), Arc::new(operation.clone())))
                .collect(),
        );
        context.child = Some(self.transport(invocation.context, false));
        context.values = invocation.values;
        context.cancellation = slot.cancellation.clone();
        Ok(context)
    }
}
impl Drop for Loan {
    fn drop(&mut self) {
        self.0.running.send_replace(false);
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
fn validate_assignments(assignments: &host::OperationAssignments) -> Result<()> {
    let mut locals = std::collections::BTreeSet::new();
    for entry in &assignments.operations {
        let local =
            entry.operation.split_once('.').map(|(_, local)| local).ok_or_else(|| invalid("Assigned Operation Is Not Qualified"))?;
        harmos_protocol::schema::operation_identity(local).map_err(|error| invalid(error.to_string()))?;
        if entry.generation == 0 || entry.fingerprint.len() != 32 || !locals.insert(local) {
            return Err(invalid("Invalid or Duplicate Guest Assignment"));
        }
    }
    Ok(())
}
/* === Tests === */
