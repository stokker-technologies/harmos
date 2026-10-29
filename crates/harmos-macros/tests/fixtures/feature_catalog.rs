/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[cfg(feature = "optional_operation")]
mod optional {
    #[harmos::operation(harmos_path = ::harmos, id = "fixture.optional", policy = super::POLICY)]
    pub struct Read;

    impl harmos::Operation for Read {
        type Error = std::convert::Infallible;

        async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<()>, Self::Error> {
            Ok(().into())
        }
    }
}


const POLICY: harmos::OperationPolicy = harmos::OperationPolicy {
    timeout: std::time::Duration::MAX,
    attempt_timeout: std::time::Duration::MAX,
    retries: harmos::RetryPolicy::Never,
};


#[harmos::operation(harmos_path = ::harmos, id = "fixture.always", policy = POLICY)]
struct Read;


#[harmos::catalog(harmos_path = ::harmos)]
enum Operations {
    Read(Read),

    #[cfg(feature = "optional_operation")]
    Optional(optional::Read),
}


impl harmos::Operation for Read {
    type Error = std::convert::Infallible;

    async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<()>, Self::Error> {
        Ok(().into())
    }
}


fn main() {
    let definitions = Operations::catalog();
    let expected = if cfg!(feature = "optional_operation") { 2 } else { 1 };
    assert_eq!(definitions.len(), expected);
    assert_eq!(definitions[0].id.as_str(), "fixture.always");
    let registration = <Operations as harmos::OperationCatalog>::batch();
    assert_eq!(registration.operations.len(), expected);
    for (definition, factory) in definitions.iter().zip(&registration.operations) {
        assert_eq!(definition.id, factory.definition.id);
        assert_eq!(definition.fingerprint, factory.definition.fingerprint);
    }
    if cfg!(feature = "optional_operation") {
        assert_eq!(definitions[1].id.as_str(), "fixture.optional");
    }
}
