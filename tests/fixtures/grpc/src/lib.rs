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
            ($ty:ty) => {{
                let value = <$ty>::decode(PAYLOAD).unwrap();
                assert_eq!(value.value, "payload");
                assert_eq!(value.encode_to_vec(), PAYLOAD);
            }};
        }
        check!(operations_messages::unary::request::Row0);
        check!(operations_messages::unary::response::Row0);
        check!(operations_messages::upload::request::Row0);
        check!(operations_messages::upload::input::Row0);
        check!(operations_messages::upload::response::Row0);
        check!(operations_messages::download::request::Row0);
        check!(operations_messages::download::output::Row0);
        check!(operations_messages::download::response::Row0);
        check!(operations_messages::exchange::request::Row0);
        check!(operations_messages::exchange::input::Row0);
        check!(operations_messages::exchange::output::Row0);
        check!(operations_messages::exchange::response::Row0);
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
