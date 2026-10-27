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

use harmos_domain::{MemorySize, schema::Described};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Debug, Default)]
pub struct InitializeRequest {
    pub config_json: Vec<u8>,
    pub generation_context: Vec<u8>,
    pub assignments: Vec<u8>,
}

#[harmos::operation(harmos_path = crate, id = "harmos_initialize", request = InitializeRequest, response = (), policy = super::POLICY, infer_request)]
pub struct Initialize {
    #[cfg(feature = "lifecycle")]
    instance: Box<dyn harmos_sdk::operations::lifecycle::LifecycleInstance>,
}

/* === Enums === */

#[cfg(feature = "lifecycle")]
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error(transparent)]
    Dependency(#[from] harmos_sdk::DependencyError),
    #[error("plugin lifecycle access failed: {0}")]
    Access(#[source] harmos_sdk::operations::access::AccessError),
}

/* === Traits === */

/* === Implementations === */

harmos_domain::postcard_product!(InitializeRequest { config_json: Vec<u8>, generation_context: Vec<u8>, assignments: Vec<u8> });
impl Described for InitializeRequest {
    const NAME: &'static str = <harmos_domain::model::operations::lifecycle::InitializationSchema as Described>::NAME;
    const FIELDS: &'static [harmos_domain::schema::declaration::Field] =
        <harmos_domain::model::operations::lifecycle::InitializationSchema as Described>::FIELDS;
    const MARKER: &'static [u8] = <harmos_domain::model::operations::lifecycle::InitializationSchema as Described>::MARKER;
}

impl MemorySize for InitializeRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.config_json.capacity())
            .saturating_add(self.generation_context.capacity())
            .saturating_add(self.assignments.capacity())
    }
}

#[cfg(feature = "lifecycle")]
impl crate::Operation for Initialize {
    type Request = InitializeRequest;
    type Response = ();
    type Error = super::ControlError;
    type Endpoints = ();
    async fn initialize(context: crate::OperationContext) -> Result<Self, Self::Error> {
        let access = context.dependency::<harmos_sdk::operations::lifecycle::LifecycleAccess>()?;
        Ok(Self { instance: access.0.clone().begin(super::Action::Initialize) })
    }
    async fn run(
        &mut self, context: crate::OperationContext, request: &mut InitializeRequest, _: &mut (),
    ) -> Result<crate::Response<()>, Self::Error> {
        let _ = &request;
        self.instance
            .run(context, &request.config_json, &request.generation_context, &request.assignments)
            .await
            .map_err(super::ControlError::Access)?;
        Ok(().into())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.instance.cleanup().await.map_err(super::ControlError::Access)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
