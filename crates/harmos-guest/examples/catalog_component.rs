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

// Third Party Imports

// Local Imports
use harmos_domain::inventory::{contracts, listeners, observers, publications};
use harmos_domain::operations::{OperationPolicy, RestartPolicy, RetryPolicy};
use harmos_guest::kit::plugin::__bindings::{
    self as bindings,
    harmos::guest::capabilities::{Refusal, Registration},
};

/* === Constants === */
const OWNER: &str = "catalog";
const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};
const SDK: [harmos_domain::operations::OperationPolicy; 2] = [POLICY, POLICY];

#[used]
static IDENTITY: [u8; contracts::identity_len(
    "guest",
    OWNER,
    "1.0.0",
    "Catalog collection fixture.",
    "Unit",
)] = contracts::encode_identity(
    "guest",
    OWNER,
    "1.0.0",
    "Catalog collection fixture.",
    "Unit",
);

#[used]
static INVENTORY: [u8; contracts::encoded_tree_len("guest", OWNER, &[], &[], &SDK)] =
    contracts::encode_tree("guest", OWNER, &[], &[], &SDK);

#[used]
static PUBLICATIONS: [u8; publications::encoded_len("guest", OWNER, &[])] =
    publications::encode("guest", OWNER, &[]);

#[used]
static LISTENERS: [u8; listeners::encoded_len("guest", OWNER, &[])] =
    listeners::encode("guest", OWNER, &[]);

#[used]
static OBSERVERS: [u8; observers::encoded_tree_len("guest", OWNER, &[], &[])] =
    observers::encode_tree("guest", OWNER, &[], &[]);

/* === Macros === */
bindings::export!(Catalog);

/* === Type Aliases === */

/* === Structs === */
struct Catalog;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
// Deliberately bypass the cooperative Registrar: the host must validate WIT
// output from arbitrary guest implementations too.
impl bindings::Guest for Catalog {
    fn register() -> Result<Vec<Registration>, String> {
        std::hint::black_box((&IDENTITY, &INVENTORY, &PUBLICATIONS, &LISTENERS, &OBSERVERS));
        let entry = |namespace: &str, schema, payload| Registration {
            namespace: namespace.into(),
            schema,
            payload,
        };
        match option_env!("HARMOS_CATALOG_CASE").unwrap_or("valid") {
            "spin" => loop {
                std::hint::spin_loop();
            },
            "memory" => {
                return Ok(vec![entry(
                    "large.catalog",
                    1,
                    vec![1u8; 128 * 1024 * 1024],
                )]);
            }
            "count" => {
                return Ok((0..257)
                    .map(|index| entry(&format!("catalog.{index}"), 1, vec![]))
                    .collect());
            }
            "bytes" => {
                return Ok(vec![entry("large.catalog", 1, vec![0u8; 17 * 1024 * 1024])]);
            }
            "namespace" => return Ok(vec![entry("not portable", 1, vec![])]),
            "schema" => return Ok(vec![entry("valid.catalog", 0, vec![])]),
            "duplicate" => {
                return Ok(vec![
                    entry("valid.catalog", 1, vec![]),
                    entry("valid.catalog", 2, vec![]),
                ]);
            }
            _ => {}
        }
        Ok(vec![
            entry("zeta.catalog", 2, br#""second""#.to_vec()),
            entry("alpha.catalog", 1, br#""first""#.to_vec()),
        ])
    }

    async fn operate(_: Vec<u8>) -> Result<Vec<u8>, Refusal> {
        panic!("operation must not run during catalog collection");
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
