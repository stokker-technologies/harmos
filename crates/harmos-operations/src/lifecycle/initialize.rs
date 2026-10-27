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
    schema::{
        Cardinality, Described,
        declaration::{Field, Kind},
    },
};
use std::borrow::Cow;

/* === Structs === */

#[derive(Clone, PartialEq, Debug, Default)]
pub struct InitializeRequest {
    pub config_json: Vec<u8>,
    pub generation_context: Vec<u8>,
    pub assignments: Vec<u8>,
}
#[harmos::operation(harmos_path = crate, id = "harmos_initialize", request = InitializeRequest, response = (), policy = super::POLICY, infer_request)]
pub struct Initialize {
    #[cfg(feature = "runtime")]
    instance: Box<dyn harmos_sdk::operations::lifecycle::LifecycleInstance>,
}

/* === Implementations === */

harmos_domain::postcard_product!(InitializeRequest { config_json: Vec<u8>, generation_context: Vec<u8>, assignments: Vec<u8> });
impl Described for InitializeRequest {
    const NAME: &'static str = "harmos.lifecycle.InitializeRequest";
    const FIELDS: &'static [Field] = &[
        Field { name: Cow::Borrowed("config_json"), tag: 1, kind: Kind::Bytes, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("generation_context"), tag: 2, kind: Kind::Bytes, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("assignments"), tag: 3, kind: Kind::Bytes, cardinality: Cardinality::Required },
    ];
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl MemorySize for InitializeRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.config_json.capacity())
            .saturating_add(self.generation_context.capacity())
            .saturating_add(self.assignments.capacity())
    }
}

#[cfg(feature = "runtime")]
impl crate::Operation for Initialize {
    type Request = InitializeRequest;
    type Response = ();
    type Error = crate::Error;
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
            .map_err(crate::Error::Access)?;
        Ok(().into())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.instance.cleanup().await.map_err(crate::Error::Access)
    }
}
