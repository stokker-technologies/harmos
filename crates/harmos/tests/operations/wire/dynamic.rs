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
use std::num::NonZeroUsize;
use std::time::Duration;
// Third Party Imports
// Local Imports
use harmos::Protobuf;
use harmos::grpc::dynamic::{Event, Operation};
use harmos::grpc::{InputEnvelope, OutputEnvelope};
use harmos::operations::{OperationPolicy, RestartPolicy, RetryPolicy, WireDeclaration};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(1),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Debug, PartialEq)]
struct Value {
    small: u32,
    wide: u64,
    optional: Option<u32>,
}

#[harmos::message]
#[harmos::operation(
    id = "dynamic.unary", response = Value, policy = POLICY,
    version = 1, grpc(package = "dynamic.v1", service = "External", method = "Unary"),
)]
struct Unary {
    small: u32,
    wide: u64,
    optional: Option<u32>,
}

#[harmos::message]
#[harmos::operation(
    id = "dynamic.exchange", response = Value, input = Value,
    output = Value, policy = POLICY, version = 1,
    grpc(package = "dynamic.v1", service = "External", method = "Exchange"),
)]
struct Exchange {
    small: u32,
    wide: u64,
    optional: Option<u32>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn contract_scenario() {
    dynamic_wire_interop();
    dynamic_stream_interop();
}

/* === Private Functions === */

/* === Tests === */
fn dynamic_wire_interop() {
    let contract = WireDeclaration::of::<Unary>().contract().unwrap();
    let operation = Operation::new(&contract, NonZeroUsize::new(4096).unwrap()).unwrap();
    let json = br#"{"small":7,"wide":"18446744073709551615","optional":0}"#;
    let payload = operation.request(json).unwrap();
    let value = Value::decode(payload.as_bytes()).unwrap();
    assert_eq!(
        value,
        Value {
            small: 7,
            wide: u64::MAX,
            optional: Some(0)
        }
    );
    assert_eq!(operation.path(), "/dynamic.v1.External/Unary");
    assert!(!operation.client_streaming());
    assert!(!operation.server_streaming());
    let Event::Completion(decoded) = operation.decode(&value.encode_to_vec()).unwrap() else {
        panic!("unary completion");
    };
    assert_eq!(decoded["wide"], "18446744073709551615");
    assert_eq!(decoded["optional"], 0);
    assert!(operation.request(br#"{"unknown":1}"#).is_err());
    assert!(operation.request(br#"{} {}"#).is_err());
    assert!(operation.input(b"{}").is_err());
    assert!(operation.decode(&[0x0a, 0xff]).is_err());
    let tiny = Operation::new(&contract, NonZeroUsize::new(4).unwrap()).unwrap();
    assert!(tiny.request(json).is_err());
}

fn dynamic_stream_interop() {
    let contract = WireDeclaration::of::<Exchange>().contract().unwrap();
    let operation = Operation::new(&contract, NonZeroUsize::new(4096).unwrap()).unwrap();
    let json = br#"{"small":7,"wide":"9"}"#;
    assert!(operation.client_streaming());
    assert!(operation.server_streaming());
    let request = operation.request(json).unwrap();
    let InputEnvelope::<Exchange, Value>::Request(value) =
        InputEnvelope::decode(request.as_bytes()).unwrap()
    else {
        panic!("request tag one");
    };
    assert_eq!(value.small, 7);
    let input = operation.input(json).unwrap();
    assert!(matches!(
        InputEnvelope::<Exchange, Value>::decode(input.as_bytes()).unwrap(),
        InputEnvelope::Item(_)
    ));
    let item = OutputEnvelope::<Value, Value>::Item(Value {
        small: 7,
        wide: 9,
        optional: None,
    })
    .encode_to_vec();
    assert!(matches!(operation.decode(&item).unwrap(), Event::Item(_)));
    let completion = OutputEnvelope::<Value, Value>::Response(Value {
        small: 8,
        wide: 9,
        optional: None,
    })
    .encode_to_vec();
    assert!(matches!(
        operation.decode(&completion).unwrap(),
        Event::Completion(_)
    ));
    assert!(operation.decode(&[]).is_err());
}
