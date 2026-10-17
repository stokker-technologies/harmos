/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

mod plugin;

#[path = "../support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use wasmtime::component::HasSelf;

// Local Imports
use harmos::guest::capabilities::Refusal as BridgeRefusal;
use harmos_domain::operations::WireChild;
use harmos_domain::{Message, Refusal};

/* === Constants === */

/* === Macros === */
wasmtime::component::bindgen!({
    path: "wit",
    world: "plugin",
    exports: { default: async },
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Stub {
    calls: Vec<Call>,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq)]
enum Call {
    Publish { stream: String, payload: Vec<u8> },
}

/* === Traits === */

/* === Implementations === */
impl harmos::guest::capabilities::Host for Stub {}

impl harmos::guest::operation_context::Host for Stub {}

impl<T: Send> harmos::guest::operation_context::HostWithStore<T> for HasSelf<Stub> {
    async fn child(
        accessor: &wasmtime::component::Accessor<T, Self>,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, BridgeRefusal> {
        accessor
            .with(|mut access| access.get().child(payload))
            .map_err(|error| BridgeRefusal {
                code: error.code().to_string(),
                failure: error.failure().encode(),
            })
    }
}

impl Stub {
    // Scripted Transport Answers Prove Encoding, Not Root Authorization
    fn child(&mut self, payload: Vec<u8>) -> Result<Vec<u8>, Refusal> {
        let WireChild::Call(call) = WireChild::decode(&payload).map_err(Refusal::of)? else {
            return Err(Refusal::new(-32601, "this stub runs no children"));
        };
        match call.operation.as_str() {
            "host.publish" => {
                let request =
                    harmos_domain::operations::streams::Publication::decode(&call.payload)
                        .map_err(Refusal::of)?;
                self.calls.push(Call::Publish {
                    stream: request.stream,
                    payload: request.payload,
                });
                Ok(().encode_to_vec())
            }
            _ => Err(Refusal::new(-32601, "no operation scope")),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
