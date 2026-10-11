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

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
// Third Party Imports

// Local Imports
use harmos::storage::{ObjectCatalog, ObjectKey};
use harmos::{Message, MessageError};
use standalone_sidecar::objects::{Calibration, Objects};

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
fn contract_scenario() {
    {
        scenario_runtime::check("validated_domain_roundtrips", || {
            let calibration = Calibration::new(7).unwrap();
            let limit = NonZeroUsize::new(4096).unwrap();
            let stored = Objects::encode(&calibration, limit).unwrap();
            assert_eq!(
                ObjectKey::new("charger:0123456789", limit)
                    .unwrap()
                    .as_str(),
                "charger:0123456789"
            );

            assert_eq!(stored.item.as_str(), "7");
            assert_eq!(
                Objects::decode::<Calibration>(&stored, limit).unwrap(),
                calibration
            );
            Objects::validate_value(&stored, limit).unwrap();
        });
    }
    {
        scenario_runtime::check("validation_survives_wire", || {
            let error = Calibration::decode(&[0]).unwrap_err();
            assert_eq!(
                error.to_string(),
                "invalid message: calibration ID must be nonzero"
            );

            let limit = NonZeroUsize::new(4096).unwrap();
            let mut stored = Objects::encode(&Calibration::new(7).unwrap(), limit).unwrap();
            stored.payload = vec![0];
            let error = Objects::decode::<Calibration>(&stored, limit).unwrap_err();
            let harmos::Error::Backend { source, .. } = error else {
                panic!("catalog decode must preserve the codec failure");
            };

            assert!(source.downcast_ref::<MessageError>().is_some());
            assert!(Objects::validate_value(&stored, limit).is_err());
            assert!(Calibration::decode(&[0xff]).is_err());
        });
    }
}
