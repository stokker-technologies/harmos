/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Unsupported Serde Wire Forms

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

#[derive(harmos::Described)]
struct Flatten {
    #[serde(flatten)]
    value: u32,
}

#[derive(harmos::Described)]
struct Serializer {
    #[serde(serialize_with = "custom")]
    value: u32,
}

#[derive(harmos::Described)]
struct Module {
    #[serde(with = "custom")]
    value: u32,
}

#[derive(harmos::Described)]
#[serde(into = "String")]
struct Conversion { value: u32 }

#[derive(harmos::Described)]
#[serde(remote = "Foreign")]
struct Remote { value: u32 }

#[derive(harmos::Described)]
struct Predicate {
    #[serde(skip_serializing_if = "custom")]
    value: u32,
}

#[derive(harmos::Described)]
struct Positional(#[serde(skip)] u32);

#[derive(harmos::Described)]
#[serde(transparent)]
struct Transparent { first: u32, second: u32 }

/* === Enums === */

#[derive(harmos::Described)]
#[serde(tag = "kind")]
enum TaggedScalar { Value(u32) }

#[derive(harmos::Described)]
enum MixedVariant {
    #[serde(untagged)]
    Value(u32),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

fn main() {}

/* === Private Functions === */

/* === Tests === */
