/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use harmos_domain::{MemorySize, schema::Described};

harmos_domain::postcard_product!(Shutdown {});

#[harmos::operation(harmos_path = crate, codec = manual, id = "harmos.shutdown", response = (), policy = super::POLICY, )]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Shutdown {}

#[cfg(feature = "lifecycle")]
pub struct Control {
    #[cfg(feature = "lifecycle")]
    instance: Box<dyn harmos_sdk::operations::lifecycle::LifecycleInstance>,
}

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
impl harmos_sdk::operations::Carrier for Control {
    type Declaration = Shutdown;
    type Error = super::ControlError;
    type StreamEndpoints = ();
    async fn bind(context: crate::ExecutionContext) -> Result<Self, Self::Error> {
        let access = context.resource::<harmos_sdk::operations::lifecycle::LifecycleAccess>()?;
        Ok(Self {
            instance: access.0.clone().begin(super::Action::Shutdown),
        })
    }
    async fn attempt(
        &mut self, context: crate::ExecutionContext, request: &mut Shutdown, _: &mut (),
    ) -> Result<crate::Response<()>, Self::Error> {
        let _ = &request;
        self.instance.run(context, &[], &[], &[]).await.map_err(super::ControlError::Access)?;
        Ok(().into())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.instance.cleanup().await.map_err(super::ControlError::Access)
    }
}

#[cfg(feature = "lifecycle")]
impl crate::Executable for Shutdown {
    type Carrier = Control;
}
