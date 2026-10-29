/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hand-Declared Payloads For Built-In Operations, Whose Fixed Tags Stay
// Independent Of The Authored Field-Name Tags.

pub mod prelude {
    pub use crate::MemorySize;
    pub use crate::schema::declaration::{Field, Kind};
    pub use crate::schema::{Cardinality, Described};
    pub use std::borrow::Cow;
}

#[doc(hidden)]
#[macro_export]
macro_rules! builtin_payload {
    ($(#[$attribute:meta])* $name:ident, $( $field:ident: $ty:ty => ($tag:literal, $kind:expr, $cardinality:ident)),* $(,)?) => {
        $(#[$attribute])*
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct $name { $(pub $field: $ty),* }
        const _: () = {
        #[allow(unused_imports)]
        use $crate::operations::payload::prelude::*;
        impl Described for $name {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($name));
            const FIELDS: &'static [Field] = &[$(Field {
                name: Cow::Borrowed(stringify!($field)), tag: $tag,
                kind: $kind, cardinality: Cardinality::$cardinality,
            }),*];
            const MARKER: &'static [u8] = &$crate::schema::declaration::encode::<{
                $crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS)
            }>(Self::NAME, Self::FIELDS);
        }
        $crate::postcard_product!($name { $($field: $ty),* });
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

pub use builtin_payload as payload;
