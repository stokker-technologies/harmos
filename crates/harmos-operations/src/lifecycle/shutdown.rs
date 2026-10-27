/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_domain::{
    MemorySize,
    schema::{Described, declaration::Field},
};

/* === Structs === */

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ShutdownRequest {}
#[harmos::operation(harmos_path = crate, id = "harmos_shutdown", request = ShutdownRequest, response = (), policy = super::POLICY, infer_request)]
pub struct Shutdown {
    #[cfg(feature = "runtime")]
    instance: Box<dyn harmos_sdk::operations::lifecycle::LifecycleInstance>,
}

/* === Implementations === */

harmos_domain::postcard_product!(ShutdownRequest {});
impl Described for ShutdownRequest {
    const NAME: &'static str = "harmos.lifecycle.ShutdownRequest";
    const FIELDS: &'static [Field] = &[];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
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
