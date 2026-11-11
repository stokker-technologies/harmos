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

harmos_domain::postcard_product!(Shutdown {});

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_operations::execution::Request)]
#[harmos_path(harmos_operations::execution)]
#[request(response = ())]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Shutdown {}

pub static SHUTDOWN_DESCRIPTION: harmos_operations::execution::operations::WireDeclaration =
    harmos_operations::execution::operations::WireDeclaration::request::<Shutdown>("harmos.shutdown", super::POLICY);

pub const SHUTDOWN_OPERATION: harmos_operations::execution::operations::OperationSpec<Shutdown> =
    harmos_operations::execution::operations::OperationSpec::<Shutdown>::new(&SHUTDOWN_DESCRIPTION);

pub struct Control {
    instance: Box<dyn crate::lifecycle::access::LifecycleInstance>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Described for Shutdown {
    const NAME: &'static str = <harmos_domain::model::operations::lifecycle::ShutdownSchema as Described>::NAME;
    const FIELDS: &'static [harmos_domain::schema::declaration::Field] =
        <harmos_domain::model::operations::lifecycle::ShutdownSchema as Described>::FIELDS;
    const MARKER: &'static [u8] = <harmos_domain::model::operations::lifecycle::ShutdownSchema as Described>::MARKER;
}

impl MemorySize for Shutdown {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl harmos_operations::execution::operations::Carrier for Control {
    type Declaration = Shutdown;
    type Error = super::ControlError;
    type StreamEndpoints = ();
    async fn bind(context: harmos_operations::execution::ExecutionContext) -> Result<Self, Self::Error> {
        let access = context.resource::<crate::lifecycle::access::LifecycleAccess>()?;
        Ok(Self {
            instance: access.0.clone().begin(super::Action::Shutdown),
        })
    }
    async fn attempt(
        &mut self, context: harmos_operations::execution::ExecutionContext, request: &mut Shutdown, _: &mut (),
    ) -> Result<harmos_operations::execution::Response<()>, Self::Error> {
        let _ = &request;
        self.instance.run(context, &[], &[], &[]).await.map_err(super::ControlError::Access)?;
        Ok(().into())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.instance.cleanup().await.map_err(super::ControlError::Access)
    }
}

/* === Public Functions === */

pub fn operation()
-> Result<harmos_operations::execution::operations::OperationFactory, harmos_operations::execution::operations::ServingError> {
    Ok(harmos_operations::execution::__private::Catalog::new().carrier::<Shutdown, Control>(SHUTDOWN_OPERATION)?.operations.remove(0))
}

/* === Private Functions === */

/* === Tests === */
