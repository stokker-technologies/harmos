/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Canonical Schema Traversal And Structural Identity

use sha2::{Digest as _, Sha256};

use super::Cardinality;
use super::graph::{Error, Kind, View};
use crate::Fingerprint;

pub fn fingerprint(root: View<'_>, codec: &str) -> Result<Fingerprint, Error> {
    let canonical = canonical(root)?;
    let mut hash = Sha256::new();
    hash.update(b"harmos.schema.v1\0");
    hash.update(u32::try_from(codec.len()).map_err(|_| Error::Budget)?.to_le_bytes());
    hash.update(codec.as_bytes());
    hash.update(canonical);
    Ok(Fingerprint(hash.finalize().into()))
}

pub fn contract(id: &str, schema: &super::graph::Schema, codec: &str) -> Result<Fingerprint, Error> {
    let root = schema.root()?;
    let direction = |name: &str| -> Result<Option<Fingerprint>, Error> {
        let Some(field) = root.fields().iter().find(|field| field.name == name) else {
            return Ok(None);
        };
        let value = root.message(field)?.ok_or_else(|| Error::Field {
            field: name.to_owned(),
            reason: "expected a message direction",
        })?;
        fingerprint(value, codec).map(Some)
    };
    let request = direction("request")?.ok_or_else(|| Error::Field {
        field: "request".into(),
        reason: "missing request",
    })?;
    let response = direction("response")?.ok_or_else(|| Error::Field {
        field: "response".into(),
        reason: "missing response",
    })?;
    Ok(operation(id, request, response, direction("input")?, direction("output")?))
}

pub fn operation(
    id: &str, request: Fingerprint, response: Fingerprint, input: Option<Fingerprint>, output: Option<Fingerprint>,
) -> Fingerprint {
    let mut hash = Sha256::new();
    hash.update(b"harmos.operation.v1\0");
    hash.update(u32::try_from(id.len()).expect("validated operation id length").to_le_bytes());
    hash.update(id.as_bytes());
    hash.update(request.0);
    hash.update(response.0);
    for direction in [input, output] {
        hash.update([u8::from(direction.is_some())]);
        if let Some(fingerprint) = direction {
            hash.update(fingerprint.0);
        }
    }
    Fingerprint(hash.finalize().into())
}

pub fn canonical(root: View<'_>) -> Result<Vec<u8>, Error> {
    let schema = root.schema();
    schema.prepare(&crate::config::SchemaSettings::default())?;
    let mut indices = vec![None; schema.definitions.len()];
    let mut order = Vec::new();
    let mut pending = vec![root.index()];
    while let Some(index) = pending.pop() {
        if indices[index].is_some() {
            continue;
        }
        indices[index] = Some(u32::try_from(order.len()).map_err(|_| Error::Budget)?);
        order.push(index);
        for field in schema.definitions[index].iter().rev() {
            if let Kind::Message { definition } = field.kind {
                pending.push(definition);
            }
        }
    }

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&u32::try_from(order.len()).map_err(|_| Error::Budget)?.to_le_bytes());
    for index in order {
        let fields = &schema.definitions[index];
        bytes.extend_from_slice(&u32::try_from(fields.len()).map_err(|_| Error::Budget)?.to_le_bytes());
        for field in fields {
            bytes.extend_from_slice(&u32::try_from(field.name.len()).map_err(|_| Error::Budget)?.to_le_bytes());
            bytes.extend_from_slice(field.name.as_bytes());
            bytes.extend_from_slice(&field.tag.to_le_bytes());
            bytes.push(match field.kind {
                Kind::Bool => 0,
                Kind::U32 => 1,
                Kind::U64 => 2,
                Kind::I32 => 3,
                Kind::I64 => 4,
                Kind::F32 => 5,
                Kind::F64 => 6,
                Kind::String => 7,
                Kind::Bytes => 8,
                Kind::Message { .. } => 9,
            });
            bytes.push(match field.cardinality {
                Cardinality::Required => 0,
                Cardinality::Optional => 1,
                Cardinality::Repeated => 2,
                Cardinality::Variant => 3,
            });
            if let Kind::Message { definition } = field.kind {
                bytes.extend_from_slice(&indices[definition].expect("visited schema edge").to_le_bytes());
            }
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Portable;
    use crate::schema::graph::{Field, Schema};

    #[test]
    fn recursive_graph_vector() {
        let first = vec![
            Field {
                name: "child".into(),
                tag: 1,
                kind: Kind::Message { definition: 1 },
                cardinality: Cardinality::Optional,
            },
            Field {
                name: "again".into(),
                tag: 2,
                kind: Kind::Message { definition: 1 },
                cardinality: Cardinality::Required,
            },
        ];
        let second = vec![Field {
            name: "back".into(),
            tag: 1,
            kind: Kind::Message { definition: 0 },
            cardinality: Cardinality::Optional,
        }];
        let schema = Schema {
            root: 0,
            definitions: vec![first, second],
        };
        let expected = fingerprint(schema.root().unwrap(), "harmos.postcard.v1").unwrap();
        assert_eq!(expected.to_string(), "c3dc086aeed6d067ea637e8648be9a969ce4ba40f4b6bd648e785dd06f6523fb");
        let mut reordered = schema.clone();
        reordered.root = 1;
        reordered.definitions.swap(0, 1);
        for fields in &mut reordered.definitions {
            for field in fields {
                if let Kind::Message { definition } = &mut field.kind {
                    *definition = 1 - *definition;
                }
            }
        }
        assert_eq!(fingerprint(reordered.root().unwrap(), "harmos.postcard.v1").unwrap(), expected);
        assert_ne!(fingerprint(schema.root().unwrap(), "custom.v1").unwrap(), expected);
        reordered.definitions.push(Vec::new());
        assert!(canonical(reordered.root().unwrap()).is_err());
    }

    #[test]
    fn stream_operation_vector() {
        let request = bool::fingerprint();
        let response = u32::fingerprint();
        let output = Some(f64::fingerprint());
        assert_eq!(request.to_string(), "20033d99f99ca5f13e227f98e440bde179835de34d6de2b56c5cf022fd0b2d56");
        assert_eq!(response.to_string(), "9f9059e4406a95ab693a629ca73e030046ebb86101ff8492544651489706ec4f");
        let identity = operation("read", request, response, None, output);
        assert_eq!(identity.to_string(), "d1c8efd88462da38b06fbb162e9ffc3a430b645f51934d8a0e3285fb3fb2e74d");
        assert_ne!(identity, operation("read", request, response, output, None));
        assert_ne!(identity, operation("read", request, response, None, None));
    }
}
