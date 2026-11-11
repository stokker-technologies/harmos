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

// Local Imports
use harmos_domain::{MemorySize, schema::Described};

/* === Constants === */

/* === Macros === */

harmos_domain::postcard_product!(Initialize { config_json: Vec<u8>, generation_context: Vec<u8>, assignments: Vec<u8> });

/* === Type Aliases === */

/* === Structs === */

#[harmos_operations::execution::operation(harmos_path = harmos_operations::execution, codec = manual, id = "harmos.initialize", response = (), policy = super::POLICY, )]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Initialize {
    pub config_json: Vec<u8>,
    pub generation_context: Vec<u8>,
    pub assignments: Vec<u8>,
}

pub struct Control {
    instance: Box<dyn crate::lifecycle::access::LifecycleInstance>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error(transparent)]
    Dependency(#[from] harmos_operations::execution::ResourceError),

    #[error("plugin lifecycle access failed: {0}")]
    Access(#[source] harmos_operations::execution::operations::access::AccessError),
}

/* === Traits === */

/* === Implementations === */

impl Described for Initialize {
    const NAME: &'static str = <harmos_domain::model::operations::lifecycle::InitializationSchema as Described>::NAME;
    const FIELDS: &'static [harmos_domain::schema::declaration::Field] =
        <harmos_domain::model::operations::lifecycle::InitializationSchema as Described>::FIELDS;
    const MARKER: &'static [u8] = <harmos_domain::model::operations::lifecycle::InitializationSchema as Described>::MARKER;
}

impl MemorySize for Initialize {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.config_json.capacity())
            .saturating_add(self.generation_context.capacity())
            .saturating_add(self.assignments.capacity())
    }
}

impl harmos_operations::execution::operations::Carrier for Control {
    type Declaration = Initialize;
    type Error = super::ControlError;
    type StreamEndpoints = ();
    async fn bind(context: harmos_operations::execution::ExecutionContext) -> Result<Self, Self::Error> {
        let access = context.resource::<crate::lifecycle::access::LifecycleAccess>()?;
        Ok(Self {
            instance: access.0.clone().begin(super::Action::Initialize),
        })
    }
    async fn attempt(
        &mut self, context: harmos_operations::execution::ExecutionContext, request: &mut Initialize, _: &mut (),
    ) -> Result<harmos_operations::execution::Response<()>, Self::Error> {
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

impl harmos_operations::execution::Executable for Initialize {
    type Carrier = Control;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
