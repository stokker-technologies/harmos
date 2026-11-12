/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Canonical Component Exports

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{Adapter, Registration};
use prost::Message as _;
use std::cell::RefCell;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

wit_bindgen::generate!({ path: "wit", world: "plugin", default_bindings_module: "crate::wasm::bindings" });
export!(Component);

thread_local! { static ADAPTER: RefCell<Option<Adapter>> = const { RefCell::new(None) }; }

/* === Type Aliases === */

/* === Structs === */

struct Component;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

// The Author Supplies This One Rust Registration Function. The SDK Owns Every ABI Export.
unsafe extern "Rust" {
    fn harmos_register() -> Registration;
}

impl Guest for Component {
    fn register() -> Result<Vec<u8>, String> {
        Ok(adapter()?.metadata().encode_to_vec())
    }
    async fn operate(owner: String, command: Vec<u8>) -> Vec<u8> {
        match adapter() {
            Ok(adapter) => adapter.operate(&owner, &command).await,
            Err(message) => harmos_protocol::v1::Response {
                result: Some(harmos_protocol::v1::response::Result::Failure(harmos_protocol::v1::Failure {
                    classification: harmos_protocol::v1::Classification::Permanent as i32,
                    code: "registration_failed".into(),
                    message,
                    ..Default::default()
                })),
            }
            .encode_to_vec(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn adapter() -> Result<Adapter, String> {
    ADAPTER.with_borrow_mut(|current| {
        if let Some(adapter) = current {
            return Ok(adapter.clone());
        }
        // The Component Is Single-Threaded And the Named Author Function Returns Owned Values.
        let registration = unsafe { harmos_register() };
        let adapter = Adapter::new(registration, |request| {
            Box::pin(async move {
                harmos::guest::operation_context::child(request).await.map_err(|bytes| {
                    harmos_protocol::v1::Failure::decode(bytes.as_slice())
                        .map(crate::Error::from)
                        .unwrap_or_else(|error| crate::Error::Invalid(error.to_string()))
                })
            })
        })
        .map_err(|error| error.to_string())?;
        *current = Some(adapter.clone());
        Ok(adapter)
    })
}

/* === Tests === */
