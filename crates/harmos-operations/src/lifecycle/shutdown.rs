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

#[harmos::operation(harmos_path = crate, codec = manual, id = "harmos.shutdown", response = (), policy = super::POLICY, )]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Shutdown {}

#[cfg(feature = "lifecycle")]
pub struct Control {
    #[cfg(feature = "lifecycle")]
    instance: Box<dyn harmos_sdk::operations::lifecycle::LifecycleInstance>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

harmos_domain::postcard_product!(Shutdown {});
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

#[cfg(feature = "lifecycle")]
impl crate::Handler for Control {
    type Request = Shutdown;
    type Response = ();
    type Error = super::ControlError;
    type StreamEndpoints = ();
    async fn initialize(context: crate::OperationContext) -> Result<Self, Self::Error> {
        let access = context.resource::<harmos_sdk::operations::lifecycle::LifecycleAccess>()?;
        Ok(Self {
            instance: access.0.clone().begin(super::Action::Shutdown),
        })
    }
    async fn run(
        &mut self, context: crate::OperationContext, request: &mut Shutdown, _: &mut (),
    ) -> Result<crate::Response<()>, Self::Error> {
        let _ = &request;
        self.instance.run(context, &[], &[], &[]).await.map_err(super::ControlError::Access)?;
        Ok(().into())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.instance.cleanup().await.map_err(super::ControlError::Access)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(feature = "lifecycle")]
impl crate::Executable for Shutdown {
    type Handler = Control;
}
