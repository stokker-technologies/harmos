/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resolver Failures and Incorrect Types Do Not Become Successful Values.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{invoke_support::*, support};
use harmos_runtime::{operations::values, plugin::grpc::Placement};
use std::sync::Arc;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Resolver(bool);
/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl values::Resolver for Resolver {
    fn get<'a>(&'a self, _: values::Request<'a>) -> values::Resolution<'a> {
        Box::pin(async move {
            if self.0 {
                return Err(harmos_runtime::Error::InvalidInput {
                    field: "Get HostValues",
                    reason: "value provider failed",
                });
            }
            Ok(harmos_protocol::context::HostValuesGetResponse {
                r#type: "harmos.context.v1.TimeGetRequest".into(),
                payload: Vec::new(),
            })
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn resolver_failure_and_a_mismatched_value_type_are_propagated() {
    let host = HostFixture::new().await;
    let plugin = host.sdk("values").await;
    let _plugin = host.runtime.attach_grpc(Placement::Remote(plugin.address.clone()), support::settings()).await.unwrap();
    let registration = host.runtime.operations.register_values(Arc::new(Resolver(false))).unwrap();
    let error = host.runtime.operations.call_encoded("fixture.value", payload(b"reading")).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(call_error(&error).to_string().contains("Get HostValues"));
    drop(registration);
    let _registration = host.runtime.operations.register_values(Arc::new(Resolver(true))).unwrap();
    let error = host.runtime.operations.call_encoded("fixture.value", payload(b"reading")).await.unwrap_err();
    assert!(call_error(&error).to_string().contains("value provider failed"), "{error:?}");
    host.runtime.shutdown().await.unwrap();
}
