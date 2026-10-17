/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![cfg(feature = "grpc")]
/* === Modules === */
pub mod contract;
mod error;

include!(concat!(env!("OUT_DIR"), "/server.rs"));
include!(concat!(env!("OUT_DIR"), "/storage_server.rs"));
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/* === Re-Exports === */
pub use error::Error;

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
#[cfg(test)]
mod tests {
    use super::fixture::v1::*;
    use prost::Message;
    const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));

    #[test]
    fn payloads_match_harmos() {
        tracing::debug!(
            bytes = PAYLOAD.len(),
            "Checking generated payload interoperability"
        );
        macro_rules! check {
            ($root:ty, $ty:ty, $field:ident) => {{
                let value = <$ty>::decode(PAYLOAD).unwrap();
                assert_eq!(value.value, "payload");
                assert_eq!(value.encode_to_vec(), PAYLOAD);
                let mut root = <$root>::default();
                root.$field = Some(value);
                let encoded = root.encode_to_vec();
                let decoded = <$root>::decode(encoded.as_slice()).unwrap();
                assert_eq!(decoded.$field.as_ref().unwrap().value, "payload");
                assert_eq!(decoded.$field.as_ref().unwrap().encode_to_vec(), PAYLOAD);
                assert_eq!(decoded.encode_to_vec(), encoded);
            }};
        }
        check!(
            operations_messages::unary::Row0,
            operations_messages::unary::Row1,
            request
        );
        check!(
            operations_messages::unary::Row0,
            operations_messages::unary::Row2,
            response
        );
        check!(
            operations_messages::upload::Row0,
            operations_messages::upload::Row1,
            request
        );
        check!(
            operations_messages::upload::Row0,
            operations_messages::upload::Row2,
            input
        );
        check!(
            operations_messages::upload::Row0,
            operations_messages::upload::Row2,
            response
        );
        check!(
            operations_messages::download::Row0,
            operations_messages::download::Row1,
            request
        );
        check!(
            operations_messages::download::Row0,
            operations_messages::download::Row2,
            output
        );
        check!(
            operations_messages::download::Row0,
            operations_messages::download::Row2,
            response
        );
        check!(
            operations_messages::exchange::Row0,
            operations_messages::exchange::Row1,
            request
        );
        check!(
            operations_messages::exchange::Row0,
            operations_messages::exchange::Row2,
            input
        );
        check!(
            operations_messages::exchange::Row0,
            operations_messages::exchange::Row2,
            output
        );
        check!(
            operations_messages::exchange::Row0,
            operations_messages::exchange::Row2,
            response
        );
    }

    #[test]
    fn envelopes_preserve_opening() {
        let opening = include_bytes!(concat!(env!("OUT_DIR"), "/opening.bin")).to_vec();
        let second = include_bytes!(concat!(env!("OUT_DIR"), "/completion.bin")).to_vec();
        let upload = OperationsUploadInput::decode(opening.as_slice()).unwrap();
        assert!(matches!(
            upload.value,
            Some(operations_upload_input::Value::Request(_))
        ));
        let download = OperationsDownloadOutput::decode(second.as_slice()).unwrap();
        assert!(matches!(
            download.value,
            Some(operations_download_output::Value::Response(_))
        ));
        let exchange = OperationsExchangeInput::decode(second.as_slice()).unwrap();
        assert!(matches!(
            exchange.value,
            Some(operations_exchange_input::Value::Item(_))
        ));
        let output = OperationsExchangeOutput::decode(opening.as_slice()).unwrap();
        assert!(matches!(
            output.value,
            Some(operations_exchange_output::Value::Item(_))
        ));
        assert_eq!(output.encode_to_vec(), opening);
    }
}
