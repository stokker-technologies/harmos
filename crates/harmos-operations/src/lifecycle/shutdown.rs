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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ShutdownRequest {}

#[harmos::operation(harmos_path = crate, id = "harmos_shutdown", request = ShutdownRequest, response = (), policy = super::POLICY, infer_request)]
pub struct Shutdown {
    #[cfg(feature = "runtime")]
    instance: Box<dyn harmos_sdk::operations::lifecycle::LifecycleInstance>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

harmos_domain::postcard_product!(ShutdownRequest {});
impl Described for ShutdownRequest {
    const NAME: &'static str = <harmos_domain::model::operations::lifecycle::ShutdownSchema as Described>::NAME;
    const FIELDS: &'static [harmos_domain::schema::declaration::Field] =
        <harmos_domain::model::operations::lifecycle::ShutdownSchema as Described>::FIELDS;
    const MARKER: &'static [u8] = <harmos_domain::model::operations::lifecycle::ShutdownSchema as Described>::MARKER;
}

impl MemorySize for ShutdownRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(feature = "runtime")]
impl crate::Operation for Shutdown {
    type Request = ShutdownRequest;
    type Response = ();
    type Error = crate::Error;
    type Endpoints = ();
    async fn initialize(context: crate::OperationContext) -> Result<Self, Self::Error> {
        let access = context.dependency::<harmos_sdk::operations::lifecycle::LifecycleAccess>()?;
        Ok(Self { instance: access.0.clone().begin(super::Action::Shutdown) })
    }
    async fn run(
        &mut self, context: crate::OperationContext, request: &mut ShutdownRequest, _: &mut (),
    ) -> Result<crate::Response<()>, Self::Error> {
        let _ = &request;
        self.instance.run(context, &[], &[], &[]).await.map_err(crate::Error::Access)?;
        Ok(().into())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.instance.cleanup().await.map_err(crate::Error::Access)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
