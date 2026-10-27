/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// TODO: Describe this module's responsibility.

/* === Modules === */

/* === Re-Exports === */

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

#[test]
fn builtin_inventory() {
    let definitions = harmos_operations::Builtins::catalog();
    let ids: std::collections::BTreeSet<_> = definitions.iter().map(|definition| definition.id.as_str()).collect();
    assert_eq!(definitions.len(), 30);
    assert_eq!(ids.len(), definitions.len());
    let mut dependencies = definitions.into_iter();
    assert!(dependencies.all(|definition| definition.fingerprint.0 != [0; 32]));
}

#[test]
fn reserved_lifecycle_fingerprints() {
    use harmos_operations::lifecycle::{Action, Descriptor, Initialize, POLICY, Shutdown};
    for (action, definition) in [(Action::Initialize, Initialize::definition()), (Action::Shutdown, Shutdown::definition())] {
        let descriptor = Descriptor::new(action, POLICY).unwrap();
        assert_eq!(descriptor.contract().operation(), &definition);
    }
}
