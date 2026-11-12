/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[test]
fn object_codec_preserves_identity_and_refuses_forged_values() {
    let first = objects_catalog().encode(&document("running"), limit()).unwrap();
    let second = objects_catalog().encode(&document("finished"), limit()).unwrap();
    assert_eq!(first.schema, second.schema);
    assert_eq!(first.item, second.item);
    assert_ne!(first.payload, second.payload);
    assert_eq!(objects_catalog().decode::<Document>(&second, limit()).unwrap(), document("finished"));
    objects_catalog().validate_value(&second, limit()).unwrap();

    let old = DifferentDocument {
        id: "document-1".into(),
        status: "archived".into(),
        annotation: 7,
    };
    let value = objects_catalog().encode(&old, limit()).unwrap();
    objects_catalog().validate_value(&value, limit()).unwrap();
    assert_eq!(objects_catalog().decode::<DifferentDocument>(&value, limit()).unwrap(), old);
    assert!(objects_catalog().decode::<Document>(&value, limit()).is_err());
    let mut unknown = value;
    unknown.schema.version = std::num::NonZeroU32::new(unknown.schema.version.get().wrapping_add(1).max(1)).unwrap();
    assert!(objects_catalog().validate_value(&unknown, limit()).is_err());

    let original = objects_catalog().encode(&document("running"), limit()).unwrap();
    for case in 0..4 {
        let mut value = original.clone();
        match case {
            0 => value.item = harmos_storage::ObjectKey::new("other", NonZeroUsize::new(4096).unwrap()).unwrap(),
            1 => value.schema.id = Identifier::new("other").unwrap(),
            2 => value.payload = vec![0xff],
            _ => value.payload = document("finished").encode(),
        }
        if case == 3 {
            // A value update with the same logical identity is valid.
            objects_catalog().validate_value(&value, limit()).unwrap();
        } else {
            assert!(objects_catalog().validate_value(&value, limit()).is_err());
            assert!(objects_catalog().decode::<Document>(&value, limit()).is_err());
        }
    }
    let invalid = Document { id: String::new(), status: "new".into() };
    assert!(objects_catalog().encode(&invalid, limit()).is_err());

    let source = document(&"x".repeat(100));
    let exact = NonZeroUsize::new(source.encoded_size()).unwrap();
    let small = NonZeroUsize::new(source.encoded_size() - 1).unwrap();
    assert!(objects_catalog().encode(&source, small).is_err());
    let value = objects_catalog().encode(&source, exact).unwrap();
    assert!(objects_catalog().decode::<Document>(&value, small).is_err());
    assert!(objects_catalog().validate_value(&value, small).is_err());
    assert_eq!(objects_catalog().decode::<Document>(&value, exact).unwrap(), source);
}
