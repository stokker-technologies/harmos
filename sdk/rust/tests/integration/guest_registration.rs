/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Inspect Guest Builder Values

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos::{Context, Operation, Plugin, Result, wasm::Registration};

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

#[test]
fn guest_registration_describes_values_before_initialization() {
    // Compose the Same Unary Builder Used by Native Plugins
    let plugin = Plugin::new("guest_fixture", "1.0.0").operation(
        Operation::new("guest.echo", async |_: &Context, _: ()| -> Result<()> { panic!("inspection entered a handler") })
            .visibility(harmos_protocol::v1::Visibility::Protected),
    );
    let registration = Registration::new(plugin)
        .capacity(2)
        .on_initialize(async |_: &Context, _: Vec<u8>| panic!("inspection initialized a plugin"))
        .on_cleanup(|| panic!("inspection entered cleanup"));

    // Read the Authoritative Catalog Without Entering Authored Code
    let metadata = registration.metadata().unwrap();
    let catalog = metadata.catalog.unwrap();
    assert_eq!(catalog.operations[0].id, "guest.echo");
    assert_eq!(catalog.operations[0].fingerprint.len(), 32);
    assert_eq!(catalog.digest, harmos_protocol::schema::catalog_digest(&catalog));

    // Refuse Invalid Builder Values Before Creating an Adapter
    assert!(Registration::new(Plugin::new("fixture", "1.0.0")).capacity(0).metadata().is_err());
    assert!(Registration::new(Plugin::new("invalid owner", "1.0.0")).metadata().is_err());
}
