/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(unsafe_code)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

#[used]
static IDENTITY: [u8; 148] = *b"\0HARMOS:ARTIFACT:V3:{\"kind\":\"guest\",\"id\":\"legacy-guest\",\"version\":\"1.0.0\",\"description\":\"A guest built before answer existed.\",\"config\":\"Settings\"}\0";

// Current Companion Fragments Cannot Make The Retired Identity Admissible
const _: () = {
    use harmos_contract::inventory::contracts::{encode_tree, encoded_tree_len};
    use harmos_contract::operations::{OperationPolicy, RestartPolicy, RetryPolicy};
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(2),
        attempt_timeout: std::time::Duration::from_secs(1),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SDK: &[harmos_contract::operations::OperationPolicy; 2] = &[POLICY, POLICY];
    #[used]
    static MANIFEST: [u8; encoded_tree_len("guest", "legacy-guest", &[], &[], SDK)] =
        encode_tree("guest", "legacy-guest", &[], &[], SDK);
};

// Explicitly no publication grant, independent of any linked row descriptors.
const _: () = {
    use harmos_contract::inventory::publications::{encode, encoded_len};
    #[used]
    static PUBLICATIONS: [u8; encoded_len("guest", "legacy-guest", &[])] =
        encode("guest", "legacy-guest", &[]);
};

/* === Macros === */
wit_bindgen::generate!({
    path: "wit",
    world: "plugin",
});

export!(LegacyComponent);

/* === Type Aliases === */

/* === Structs === */
struct LegacyComponent;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Guest for LegacyComponent {
    fn register() -> Result<Vec<harmos::guest::capabilities::Registration>, String> {
        panic!("retired metadata must be rejected before registration")
    }
    async fn operate(_: Vec<u8>) -> Result<Vec<u8>, harmos::guest::capabilities::Refusal> {
        panic!("retired metadata must be rejected before SDK execution")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
