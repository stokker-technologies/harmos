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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[facade::message(harmos_path = ::facade::__harmos)]
#[derive(Clone, Debug)]
struct WireRequest {
    count: u64,
}

struct ExportRequest(WireRequest);

impl facade::__harmos::Message for ExportRequest {
    const CODEC: &'static str = <WireRequest as facade::__harmos::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        facade::__harmos::Message::encode(&self.0)
    }
    fn encode_into(&self, bytes: &mut Vec<u8>) {
        facade::__harmos::Message::encode_into(&self.0, bytes);
    }
    fn encoded_size(&self) -> usize {
        facade::__harmos::Message::encoded_size(&self.0)
    }
    fn decode(bytes: &[u8]) -> Result<Self, facade::__harmos::MessageError> {
        <WireRequest as facade::__harmos::Message>::decode(bytes).map(Self)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, facade::__harmos::MessageError> {
        <WireRequest as facade::__harmos::Message>::decoding_memory(bytes)
    }
}

impl facade::__harmos::MemorySize for ExportRequest {
    fn memory_size(&self) -> usize {
        facade::__harmos::MemorySize::memory_size(&self.0)
    }
}

impl facade::__harmos::schema::Described for ExportRequest {
    const NAME: &'static str = "ExportRequest";
    const FIELDS: &'static [facade::__harmos::schema::graph::Field<facade::__harmos::schema::declaration::Definition>] =
        <WireRequest as facade::__harmos::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = &facade::__harmos::schema::declaration::encode::<
        { facade::__harmos::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

#[facade::operation(
    harmos_path = ::facade::__harmos, id = "export",
    request = ExportRequest, response = (),
    policy = facade::__harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(1),
    retries: facade::__harmos::operations::RetryPolicy::Never,
},
)]
struct Export {
    #[cfg(feature = "runtime")]
    resource: ::std::sync::Arc<RuntimeResource>,
}

#[cfg(feature = "runtime")]
struct RuntimeResource;

#[facade::operation(harmos_path = ::facade::__harmos, request = ExportRequest, response = (),
    policy = facade::__harmos::operations::OperationPolicy {
        timeout: std::time::Duration::from_secs(30),
        attempt_timeout: std::time::Duration::from_secs(1),
        retries: facade::__harmos::operations::RetryPolicy::Never,
    })]
struct ExportAgain;

/* === Enums === */

#[facade::catalog(harmos_path = ::facade::__harmos)]
enum Operations {
    Export(Export),
    ExportAgain(ExportAgain),
}

#[facade::catalog(harmos_path = ::facade::__harmos)]
enum NestedOperations {
    #[nested]
    Operations(Operations),
}

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl facade::__harmos::Operation for Export {
    type Request = ExportRequest;
    #[cfg(not(feature = "mismatched_payload"))]
    type Response = ();
    #[cfg(feature = "mismatched_payload")]
    type Response = u64;
    type Error = ::core::convert::Infallible;
    type Endpoints = ();

    async fn initialize(_: facade::__harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { resource: ::std::sync::Arc::new(RuntimeResource) })
    }
    async fn run(
        &mut self, _: facade::__harmos::OperationContext, _: &mut ExportRequest, _: &mut (),
    ) -> Result<facade::__harmos::Response<Self::Response>, Self::Error> {
        Ok(Self::Response::default().into())
    }
}

#[cfg(feature = "runtime")]
impl facade::__harmos::Operation for ExportAgain {
    type Request = ExportRequest;
    type Response = ();
    type Error = ::core::convert::Infallible;
    type Endpoints = ();

    async fn initialize(_: facade::__harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self)
    }
    async fn run(
        &mut self, _: facade::__harmos::OperationContext, _: &mut ExportRequest, _: &mut (),
    ) -> Result<facade::__harmos::Response<()>, Self::Error> {
        Ok(().into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let entries = NestedOperations::catalog();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id.as_str(), "export");
    #[cfg(feature = "runtime")]
    let _ = NestedOperations::registration(facade::__harmos::Dependencies::new());
}

/* === Tests === */
