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
pub mod declaration;
pub mod graph;
mod payload;

#[cfg(feature = "grpc")]
mod protobuf;


/* === Re-Exports === */
pub use declaration::{Field, Kind};
pub use payload::{Event, PathElement, PayloadError, Scalar, Validator, VisitError};

#[cfg(feature = "grpc")]
pub use protobuf::{PayloadError as ProtobufPayloadError, Validator as ProtobufValidator};

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports

/* === Constants === */
pub const MAX_DEPTH: usize = 32;
pub const MAX_NAME_BYTES: usize = 256;

/* === Macros === */
// Actual Prost Scalar Wrapper Payloads, With Their Fixed Field-One Identity
macro_rules! wrapper {
    ($($ty:ty => $kind:ident),* $(,)?) => {$(
        impl Described for $ty {
            const NAME: &'static str = concat!("harmos::", stringify!($ty));
            const FIELDS: &'static [graph::Field<declaration::Definition>] = &[graph::Field {
                name: Cow::Borrowed("value"),
                tag: 1,
                kind: graph::Kind::$kind,
                cardinality: Cardinality::Required,
            }];
            const MARKER: &'static [u8] = &declaration::encode::<{
                declaration::encoded_len(Self::NAME, Self::FIELDS)
            }>(Self::NAME, Self::FIELDS);
        }
    )*};
}

wrapper!(bool => Bool, u32 => U32, u64 => U64, i32 => I32, i64 => I64, f32 => F32, f64 => F64, String => String, Vec<u8> => Bytes);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    Required,
    Optional,
    Repeated,
    Variant,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Graph(#[from] graph::Error),
}

/* === Traits === */
// A Static Message Shape, Independent Of Payload Encoding Or Construction.
pub trait Described: 'static {
    const NAME: &'static str;
    const FIELDS: &'static [graph::Field<declaration::Definition>];
    const MARKER: &'static [u8];
    const DEFINITION: declaration::Definition = declaration::Definition::of::<Self>();
}

/* === Implementations === */
impl Described for () {
    const NAME: &'static str = "harmos::unit";
    const FIELDS: &'static [graph::Field<declaration::Definition>] = &[];
    const MARKER: &'static [u8] = &declaration::encode::<
        { declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

/* === Public Functions === */

/* === Private Functions === */
fn diagnostic_name(name: &str) -> String {
    if name.len() <= MAX_NAME_BYTES {
        return name.to_owned();
    }
    let mut end = MAX_NAME_BYTES - 3;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = String::with_capacity(MAX_NAME_BYTES);
    result.push_str(&name[..end]);
    result.push_str("...");
    result
}

/* === Tests === */
