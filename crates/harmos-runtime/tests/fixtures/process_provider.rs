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
use contract::{POLICY, Read, Readings, Step};
use harmos_runtime::operations::{Operation, OperationPolicy, WireTable};
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

struct Reading {
    resources: Arc<Resources>,
    phase: Option<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::operations::lifecycle::Policy for Provider {
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
            .implementation::<contract::ReadDeclaration, Reading>(),
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

impl Operation for Reading {
    type Request = <contract::ReadDeclaration as harmos_runtime::OperationDeclaration>::Request;
    type Response = <contract::ReadDeclaration as harmos_runtime::OperationDeclaration>::Response;

    type Error = Refusal;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Refusal> {
        let resources = context.dependency::<Resources>().expect("fixture dependency");
        let constructed: Result<Self, Refusal> = async move { Ok(Self { resources, phase: None }) }.await;
        let mut operation = constructed?;
        let started: Result<(), Refusal> = async {
            let operation = &mut operation;
            operation.resources.marks.mark("read-initialized");
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }

    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut Read, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Step>, Refusal> {
        let result: ::core::result::Result<Step, Self::Error> = async move {
            self.phase = Some(request.phase);
            self.resources.marks.mark(&format!("read-prepared-{}", request.phase));
            if !self.resources.initialized.load(Ordering::SeqCst) {
                return Err(Refusal::of("the original provider has retired"));
            }
            self.resources.marks.mark(&format!("read-started-{}", request.phase));
            if request.phase == 2 && self.resources.marks.seen("hold-read-2") {
                self.resources.marks.marked("release-read-2").await;
            }
            Ok(Step { phase: request.phase })
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        if !self.resources.initialized.load(Ordering::SeqCst) {
            return Err(Refusal::of("the provider retired before child finalization"));
        }
        match self.phase {
            Some(phase) => self.resources.marks.mark(&format!("read-finalized-{phase}")),
            None => self.resources.marks.mark("read-finalized-unstarted"),
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
