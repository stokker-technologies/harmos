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

#[facade::message]
#[derive(Clone, Debug)]
struct WireRequest {
    count: u64,
}

#[facade::operation(
    id = "test.export",
    codec = manual, resources = [RuntimeResource],
    policy = facade::__harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(1),
    retries: facade::__harmos::operations::RetryPolicy::Never,
},
)]
#[derive(Clone)]
struct Export(WireRequest);

impl facade::__harmos::Message for Export {
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

impl facade::__harmos::MemorySize for Export {
    fn memory_size(&self) -> usize {
        facade::__harmos::MemorySize::memory_size(&self.0)
    }
}

impl facade::__harmos::schema::Described for Export {
    const NAME: &'static str = "Export";
    const FIELDS: &'static [facade::__harmos::schema::graph::Field<facade::__harmos::schema::declaration::Definition>] =
        <WireRequest as facade::__harmos::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = &facade::__harmos::schema::declaration::encode::<
        { facade::__harmos::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

#[cfg(feature = "runtime")]
struct RuntimeResource;

#[facade::operation(id = "test.export_again", policy = <Export as facade::__harmos::OperationDeclaration>::POLICY)]
struct ExportAgain;

/* === Enums === */

#[facade::catalog]
enum Operations {

    #[cfg(any())]
    Missing(unresolved::Operation),

    Export(Export),
    ExportAgain(ExportAgain),
}

#[facade::catalog]
enum NestedOperations {
    #[nested]
    Operations(Operations),
}

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl facade::__harmos::Operation for Export {
    type Error = ::core::convert::Infallible;
    #[cfg(not(feature = "mismatched_payload"))]
    async fn run(self, ctx: facade::__harmos::Context<'_, Self>) -> Result<facade::__harmos::Response<()>, Self::Error> {
        let _ = ctx.resource::<RuntimeResource>();
        Ok(().into())
    }
    #[cfg(feature = "mismatched_payload")]
    async fn run(self, _: facade::__harmos::Context<'_, Self>) -> Result<facade::__harmos::Response<u64>, Self::Error> {
        Ok(0.into())
    }
}

#[cfg(feature = "runtime")]
impl facade::__harmos::Operation for ExportAgain {
    type Error = ::core::convert::Infallible;
    async fn run(self, _: facade::__harmos::Context<'_, Self>) -> Result<facade::__harmos::Response<()>, Self::Error> {
        Ok(().into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let entries = NestedOperations::catalog();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id.as_str(), "test.export");
    #[cfg(feature = "runtime")]
    let _ = <NestedOperations as facade::__harmos::operations::Catalog>::batch();
}

/* === Tests === */
