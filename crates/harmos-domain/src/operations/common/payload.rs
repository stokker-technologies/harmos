/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hand-Declared Payloads For Built-In Operations, Whose Historical Tags Stay
// Independent Of The Authored Field-Name Tags.

/* === Modules === */
pub(crate) mod prelude {
    pub(crate) use crate::MemorySize;
    pub(crate) use crate::schema::declaration::{Field, Kind};
    pub(crate) use crate::schema::{Cardinality, Described};
    pub(crate) use std::borrow::Cow;
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */
macro_rules! payload {
    ($name:ident, $( $field:ident: $ty:ty => ($tag:literal, $kind:expr, $cardinality:ident)),* $(,)?) => {
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct $name { $(pub $field: $ty),* }
        const _: () = {
        #[allow(unused_imports)]
        use $crate::operations::common::payload::prelude::*;
        impl Described for $name {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($name));
            const FIELDS: &'static [Field] = &[$(Field {
                name: Cow::Borrowed(stringify!($field)), tag: $tag,
                kind: $kind, cardinality: Cardinality::$cardinality,
            }),*];
            const MARKER: &'static [u8] = &crate::schema::declaration::encode::<{
                crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS)
            }>(Self::NAME, Self::FIELDS);
        }
        crate::message::postcard_product!($name { $($field: $ty),* });
        impl MemorySize for $name {
            fn memory_size(&self) -> usize {
                std::mem::size_of::<Self>()$(.saturating_add(
                    self.$field.memory_size().saturating_sub(std::mem::size_of::<$ty>())
                ))*
            }
        }
        };
    };
}

pub(crate) use payload;

// Keep Historical Host Tags Independent Of Authored Field-Name Tags

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
// Field Memory Excludes Inline Storage Already Charged By The Payload

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
