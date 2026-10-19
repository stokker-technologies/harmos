/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Enum Variant Selection Against A Newer Twin Schema

/* === Modules === */

// The Deployed Schema Knows Two Modes
mod known {
    use harmos_domain::message;

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub enum Mode {
        Automatic,
        Manual,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, Default, PartialEq)]
    pub enum Preferred {
        Automatic,

        #[default]
        Manual,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Run {
        pub id: u32,
        pub mode: Mode,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Boxed {
        pub id: u32,
        pub mode: Box<Mode>,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub enum Wrapped {
        Hold(Box<Mode>),
    }
}

// A Newer Client Adds A Third Mode Under The Same Names
mod newer {
    use harmos_domain::message;

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub enum Mode {
        Automatic,
        Manual,
        Remote,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Run {
        pub id: u32,
        pub mode: Mode,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub enum Wrapped {
        Hold(Box<Mode>),
    }

    // Omits The Mode Field Entirely
    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Bare {
        pub id: u32,
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt::Debug;
// Third Party Imports

// Local Imports
use harmos_domain::Message;
use harmos_sdk::grpc::{self, DecodeInput};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[test]
fn unknown_variant_refused() {
    let remote = wire(&newer::Mode::Remote);
    refused::<known::Mode>(&remote);
    refused::<known::Preferred>(&remote);
    let run = wire(&newer::Run { id: 7, mode: newer::Mode::Remote });
    refused::<known::Run>(&run);
    refused::<known::Boxed>(&run);
    let wrapped = wire(&newer::Wrapped::Hold(Box::new(newer::Mode::Remote)));
    refused::<known::Wrapped>(&wrapped);
}

#[test]
fn absent_variant_defaults() {
    assert_eq!(read::<known::Mode>(&[]), known::Mode::Automatic);
    assert_eq!(read::<known::Preferred>(&[]), known::Preferred::Manual);
    let bare = wire(&newer::Bare { id: 7 });
    let run = read::<known::Run>(&bare);
    assert_eq!(run.mode, known::Mode::Automatic);
    assert_eq!(*read::<known::Boxed>(&bare).mode, run.mode);
    grpc::decoding_memory::<known::Run>(DecodeInput::new(&bare)).unwrap();
}

#[test]
fn known_variant_decodes() {
    let manual = wire(&newer::Mode::Manual);
    assert_eq!(read::<known::Mode>(&manual), known::Mode::Manual);
    let run = wire(&newer::Run { id: 7, mode: newer::Mode::Manual });
    assert_eq!(read::<known::Run>(&run).mode, known::Mode::Manual);
    assert_eq!(*read::<known::Boxed>(&run).mode, known::Mode::Manual);
    let wrapped = wire(&newer::Wrapped::Hold(Box::new(newer::Mode::Manual)));
    assert_eq!(read::<known::Wrapped>(&wrapped), known::Wrapped::Hold(Box::new(known::Mode::Manual)));
}

#[test]
fn segments_keep_selection() {
    let manual = wire(&newer::Mode::Manual);
    let remote = wire(&newer::Mode::Remote);
    // A Known Variant Never Masks An Unknown One In Either Order
    refused::<known::Mode>(&[manual.clone(), remote.clone()].concat());
    refused::<known::Mode>(&[remote, manual.clone()].concat());
    // Declared Duplicates Keep Last-Wins Selection
    let automatic = wire(&newer::Mode::Automatic);
    assert_eq!(read::<known::Mode>(&[automatic, manual].concat()), known::Mode::Manual);
    // Parent Field Occurrences Merge Before Variant Selection
    let run = |mode| wire(&newer::Run { id: 7, mode });
    let segmented = [run(newer::Mode::Manual), run(newer::Mode::Remote)].concat();
    refused::<known::Run>(&segmented);
    refused::<known::Boxed>(&segmented);
}

#[test]
fn enum_postcard_parity() {
    let remote = Message::encode_to_vec(&newer::Mode::Remote);
    assert!(<known::Mode as Message>::decode(&remote).is_err());
    refused::<known::Mode>(&wire(&newer::Mode::Remote));
    let manual = Message::encode_to_vec(&newer::Mode::Manual);
    assert_eq!(<known::Mode as Message>::decode(&manual).unwrap(), known::Mode::Manual);
}

/* === Private Functions === */

fn wire<T: harmos_domain::Message + harmos_domain::schema::Described>(value: &T) -> Vec<u8> {
    crate::encode(value)
}

fn read<T: harmos_domain::Message + harmos_domain::schema::Described>(bytes: &[u8]) -> T {
    grpc::decode::<T>(DecodeInput::new(bytes)).unwrap()
}

fn refused<T: harmos_domain::Message + harmos_domain::schema::Described + Debug>(bytes: &[u8]) {
    let error = grpc::decode::<T>(DecodeInput::new(bytes)).unwrap_err();
    assert!(error.to_string().contains("unknown protobuf enum variant"), "{error}");
    assert!(grpc::decoding_memory::<T>(DecodeInput::new(bytes)).is_err());
}

/* === Tests === */
