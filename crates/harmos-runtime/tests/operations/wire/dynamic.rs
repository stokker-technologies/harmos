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
use harmos_runtime::grpc::dynamic::{Event, Operation};
use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope};
use harmos_runtime::operations::{OperationPolicy, RetryPolicy, WireDeclaration};
use harmos_sdk::grpc::{self, DecodeInput};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(1),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
#[derive(Debug, PartialEq)]
struct Value {
    small: u32,
    wide: u64,
    optional: Option<u32>,
}

#[harmos_runtime::operation(id = "dynamic_unary", response = Value, policy = POLICY,
     grpc(package = "dynamic.v1", service = "External", method = "Unary"),
)]
struct UnaryDeclaration {
    small: u32,
    wide: u64,
    optional: Option<u32>,
}

#[harmos_runtime::operation(id = "dynamic_exchange", response = Value, input = Value,
    output = Value, policy = POLICY,
    grpc(package = "dynamic.v1", service = "External", method = "Exchange"),
)]
struct ExchangeDeclaration {
    small: u32,
    wide: u64,
    optional: Option<u32>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn dynamic_wire_interop() {
    let contract = WireDeclaration::of::<UnaryDeclaration>().contract().unwrap();
    let operation = Operation::new(&contract, NonZeroUsize::new(4096).unwrap()).unwrap();
    let json = br#"{"small":7,"wide":"18446744073709551615","optional":0}"#;
    let payload = operation.request(json).unwrap();
    let value = grpc::decode::<Value>(DecodeInput::new(payload.as_bytes())).unwrap();
    assert_eq!(
        value,
        Value {
            small: 7,
            wide: u64::MAX,
            optional: Some(0)
        }
    );
    assert_eq!(operation.path(), "/dynamic.v1.External/UnaryDeclaration");
    assert!(!operation.client_streaming());
    assert!(!operation.server_streaming());
    let Event::Completion(decoded) = operation
        .decode(&{
            let mut bytes = Vec::new();
            grpc::encode(&(value), &mut bytes).unwrap();
            bytes
        })
        .unwrap()
    else {
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

#[test]
fn dynamic_stream_interop() {
    let contract = WireDeclaration::of::<ExchangeDeclaration>().contract().unwrap();
    let operation = Operation::new(&contract, NonZeroUsize::new(4096).unwrap()).unwrap();
    let json = br#"{"small":7,"wide":"9"}"#;
    assert!(operation.client_streaming());
    assert!(operation.server_streaming());
    let request = operation.request(json).unwrap();
    let InputEnvelope::<ExchangeDeclaration, Value>::Request(value) = <harmos_runtime::grpc::Transcoded as harmos_runtime::grpc::Wire<
        InputEnvelope<ExchangeDeclaration, Value>,
    >>::decode_admitted(DecodeInput::new(request.as_bytes()), |_| {
        Ok(())
    })
    .unwrap()
    .0
    else {
        panic!("request tag one");
    };
    assert_eq!(value.small, 7);
    let input = operation.input(json).unwrap();
    assert!(matches!(
        <harmos_runtime::grpc::Transcoded as harmos_runtime::grpc::Wire<InputEnvelope<ExchangeDeclaration, Value>>>::decode_admitted(
            DecodeInput::new(input.as_bytes()),
            |_| Ok(())
        )
        .unwrap()
        .0,
        InputEnvelope::Item(_)
    ));
    let item = {
        let mut bytes = Vec::new();
        <harmos_runtime::grpc::Transcoded as harmos_runtime::grpc::Wire<OutputEnvelope<Value, Value>>>::encode(
            &(OutputEnvelope::<Value, Value>::Item(Value { small: 7, wide: 9, optional: None })),
            &mut bytes,
        )
        .unwrap();
        bytes
    };
    assert!(matches!(operation.decode(&item).unwrap(), Event::Item(_)));
    let completion = {
        let mut bytes = Vec::new();
        <harmos_runtime::grpc::Transcoded as harmos_runtime::grpc::Wire<OutputEnvelope<Value, Value>>>::encode(
            &(OutputEnvelope::<Value, Value>::Response(Value { small: 8, wide: 9, optional: None })),
            &mut bytes,
        )
        .unwrap();
        bytes
    };
    assert!(matches!(operation.decode(&completion).unwrap(), Event::Completion(_)));
    assert!(operation.decode(&[]).is_err());
}
