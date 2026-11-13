/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Product Value Resolution Is Declared, Scoped and Owned by Its Registration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{invoke_support::*, support};
use harmos_runtime::{operations::values, plugin::grpc::Placement};
use prost::Message;
use std::sync::{Arc, Mutex};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Resolver(Arc<Mutex<Vec<(String, String, String)>>>);
/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl values::Resolver for Resolver {
    fn get<'a>(&'a self, request: values::Request<'a>) -> values::Resolution<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push((
                request.plugin.into(),
                request.declaration.name.clone(),
                request.context.get::<harmos_runtime::Scope>().unwrap().as_str().into(),
            ));
            Ok(harmos_protocol::context::HostValuesGetResponse {
                r#type: request.declaration.r#type.clone(),
                payload: harmos_protocol::context::TimeGetResponse {
                    now: Some(prost_types::Timestamp { seconds: 17, nanos: 0 }),
                }
                .encode_to_vec(),
            })
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn declared_values_use_the_product_resolver_and_registration_lifetime() {
    let host = HostFixture::new().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let registration = host.runtime.operations.register_values(Arc::new(Resolver(seen.clone()))).unwrap();
    let plugin = host.sdk("values").await;
    let _plugin = host.runtime.attach_grpc(Placement::Remote(plugin.address.clone()), support::settings()).await.unwrap();
    let scoped = host.runtime.operations.with_scope("opaque selection").unwrap();
    let response = scoped.call_encoded("fixture.value", payload(b"reading")).await.unwrap();
    #[derive(prost::Message)]
    struct Value {
        #[prost(bytes = "vec", tag = "1")]
        value: Vec<u8>,
    }
    let response = Value::decode(response.as_slice()).unwrap();
    assert_eq!(harmos_protocol::context::TimeGetResponse::decode(response.value.as_slice()).unwrap().now.unwrap().seconds, 17);
    assert_eq!(*seen.lock().unwrap(), [("values".into(), "reading".into(), "opaque selection".into())]);

    // Undeclared Values Never Reach the Product Resolver.
    let error = scoped.call_encoded("fixture.value", payload(b"undeclared")).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(call_error(&error).to_string().contains("Get HostValues"));
    assert_eq!(seen.lock().unwrap().len(), 1);

    // Dropping the Registration Ends New Resolution Without a Library Value Store.
    drop(registration);
    let error = scoped.call_encoded("fixture.value", payload(b"reading")).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert_eq!(seen.lock().unwrap().len(), 1);
    host.runtime.shutdown().await.unwrap();
}
