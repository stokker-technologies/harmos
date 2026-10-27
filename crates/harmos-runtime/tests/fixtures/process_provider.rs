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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
// Third Party Imports

// Local Imports
use super::contract;
use crate::sidecar::witness::Witness;
use contract::{POLICY, ReadDeclaration, Readings, Step};
use harmos_runtime::operations::{OperationPolicy, WireTable};
use harmos_runtime::sidecar::{Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Provider {
    resources: Arc<Resources>,
}

pub(crate) struct Resources {
    marks: Arc<Witness>,
    initialized: AtomicBool,
}

#[harmos_runtime::operation(codec = manual, resources = [Resources], id = <ReadDeclaration as harmos_runtime::OperationDeclaration>::ID, response = Step,
    policy = POLICY, visibility = Shared)]
#[derive(Clone)]
struct Reading(ReadDeclaration);

impl harmos_runtime::Message for Reading {
    const CODEC: &'static str = <ReadDeclaration as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <ReadDeclaration as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <ReadDeclaration as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for Reading {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for Reading {
    const NAME: &'static str = <ReadDeclaration as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <ReadDeclaration as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <ReadDeclaration as harmos_runtime::schema::Described>::MARKER;
}

struct AttemptGuard {
    resources: Arc<Resources>,
    phase: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_operations::lifecycle::Policy for Provider {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos_runtime::plugin(id = "process_provider", provides = Readings, requires = ())]
impl harmos_runtime::Plugin for Provider {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Resources;

    fn prepare(key: String) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Resources {
                marks: Witness::find(&key).ok_or_else(|| Refusal::of("the story witness is gone"))?,
                initialized: AtomicBool::new(false),
            }),
        })
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.marks.mark("initialized");
        self.resources.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(&self, table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<Reading>(),
        )
    }

    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.resources.initialized.store(false, Ordering::SeqCst);
        self.resources.marks.mark("shutdown");
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        self.resources.marks.mark("cleaned");
        Ok(())
    }
}

impl harmos_runtime::Operation for Reading {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Step>, Refusal> {
        let resources = ctx.resource::<Resources>();
        let request = self.0;
        resources.marks.mark("read-initialized");
        let _guard = AttemptGuard {
            resources: resources.clone(),
            phase: request.phase,
        };
        let result: ::core::result::Result<Step, Self::Error> = async move {
            resources.marks.mark(&format!("read-prepared-{}", request.phase));
            if !resources.initialized.load(Ordering::SeqCst) {
                return Err(Refusal::of("the original provider has retired"));
            }
            resources.marks.mark(&format!("read-started-{}", request.phase));
            if request.phase == 2 && resources.marks.seen("hold-read-2") {
                resources.marks.marked("release-read-2").await;
            }
            Ok(Step { phase: request.phase })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        assert!(self.resources.initialized.load(Ordering::SeqCst));
        self.resources.marks.mark(&format!("read-finalized-{}", self.phase));
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
