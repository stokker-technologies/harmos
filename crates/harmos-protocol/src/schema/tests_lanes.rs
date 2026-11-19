/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Additive Plugin Lane Descriptions.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use crate::model::{LaneDeclaration, OperationContract, lane_declaration};

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
fn absent_lane_fields_preserve_original_description_bytes_and_digest() {
    // Bytes Written With the Original Six-Field Description Schema.
    let bytes = [0x0a, 0x0e, 0x0a, 0x0c, b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'r', b'e', b'a', b'd'];
    let declarations = Declarations::decode(bytes.as_slice()).unwrap();
    assert_eq!(declarations.encode_to_vec(), bytes);
    assert_eq!(declarations_digest(&declarations), Sha256::digest(bytes).to_vec());
    let mut changed = declarations.clone();
    changed.lanes.push(LaneDeclaration {
        name: "instrument".into(),
        capacity: Some(lane_declaration::Capacity::Serial(true)),
    });
    changed.operations[0].lane = "instrument".into();
    assert_ne!(declarations_digest(&declarations), declarations_digest(&changed));
}

#[test]
fn lane_assignments_validate_lifecycle_and_membership() {
    let lane = LaneDeclaration {
        name: "instrument".into(),
        capacity: Some(lane_declaration::Capacity::Serial(true)),
    };
    let mut declarations = Declarations {
        lanes: vec![lane.clone()],
        operations: vec![OperationContract {
            id: "example.initialize".into(),
            lane: lane.name.clone(),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(plugin_lanes("example", &declarations).is_err());
    declarations.operations[0].id = "example.measure".into();
    assert!(plugin_lanes("example", &declarations).is_ok());
    declarations.operations[0].lane = "undeclared".into();
    assert!(plugin_lanes("example", &declarations).is_err());
}
