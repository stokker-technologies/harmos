/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
// Third Party Imports

// Local Imports
use harmos_contract::journal::Position;
use harmos_contract::listeners::{Context, Delivery};
use harmos_contract::{Protobuf, message};

/* === Constants === */
#[global_allocator]
static ALLOCATOR: Tracking = Tracking;

/* === Macros === */
thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
    static MEASURED: Cell<Measurement> = const { Cell::new(Measurement { allocations: 0, live: 0, peak: 0 }) };
}

macro_rules! layer {
    ($name:ident, $inner:ident) => {
        #[message(harmos_path = ::harmos_contract)]
        #[derive(PartialEq)]
        struct $name {
            row: $inner,
        }
    };
}

layer!(First, Leaf);

layer!(Second, First);

layer!(Third, Second);

layer!(Fourth, Third);

layer!(Fifth, Fourth);

/* === Type Aliases === */

/* === Structs === */
struct Tracking;

#[derive(Clone, Copy, Default, Debug)]
struct Measurement {
    allocations: usize,
    live: usize,
    peak: usize,
}

struct MeasurementGuard;

#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
struct Leaf {
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
struct Selection {
    row: Choice,
    optional: Option<Fifth>,
    rows: Vec<Fifth>,
}

#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
struct Strings {
    label: String,
    optional: Option<String>,
    labels: Vec<String>,
    counts: Vec<u32>,
}

#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
struct BoxedSelection {
    row: Box<Fifth>,
    choice: BoxedChoice,
}

#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
struct BoxedLeaf {
    row: Box<Leaf>,
}

/* === Enums === */
#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
enum BoxedChoice {
    Selected(Box<Fifth>),
    Named { row: Box<Leaf> },
}

#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
enum Choice {
    Selected(Fifth),
    Empty,
}

/* === Traits === */

/* === Implementations === */
// A Pass-Through Allocator Measures Only This Test Thread's Encoding Buffers
#[allow(
    unsafe_code,
    reason = "observing allocation peaks requires an allocator shim"
)]
unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            account(0, layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        account(layout.size(), 0);
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            account(layout.size(), size);
        }
        next
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            account(0, layout.size());
        }
        pointer
    }
}

impl Drop for MeasurementGuard {
    fn drop(&mut self) {
        MEASURING.set(false);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn account(released: usize, allocated: usize) {
    if !MEASURING.get() {
        return;
    }
    let mut measured = MEASURED.get();
    measured.allocations = measured
        .allocations
        .saturating_add(usize::from(allocated != 0));
    // A Caller Buffer May Have Been Allocated Before Measurement Started
    measured.live = measured
        .live
        .saturating_sub(released)
        .saturating_add(allocated);
    measured.peak = measured.peak.max(measured.live);
    MEASURED.set(measured);
}

fn measure<T>(encode: impl FnOnce() -> T) -> (T, Measurement) {
    MEASURED.set(Measurement::default());
    MEASURING.set(true);
    let guard = MeasurementGuard;
    let value = encode();
    drop(guard);
    (value, MEASURED.get())
}

fn nested() -> Fifth {
    Fifth {
        row: Fourth {
            row: Third {
                row: Second {
                    row: First {
                        row: Leaf {
                            payload: vec![255; 32_768],
                        },
                    },
                },
            },
        },
    }
}

fn selection() -> Selection {
    Selection {
        row: Choice::Selected(nested()),
        optional: Some(nested()),
        rows: vec![nested(), nested()],
    }
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("rows_allocate_final", || {
            let row = selection();
            let (encoded, measured) = measure(|| row.encode_to_vec());
            assert_eq!(measured.allocations, 1, "{measured:?}");
            assert_eq!(measured.peak, encoded.capacity());
            assert_eq!(encoded.len(), row.encoded_len());
            assert_eq!(Selection::decode(&encoded).unwrap(), row);

            let mut appended = Vec::with_capacity(1 + row.encoded_len());
            appended.push(42);
            let (_, measured) = measure(|| row.encode_into(&mut appended));
            assert_eq!(measured.allocations, 0, "{measured:?}");
            assert_eq!(appended[0], 42);
            assert_eq!(&appended[1..], encoded);

            let mut growing = vec![42];
            let (_, measured) = measure(|| row.encode_into(&mut growing));
            assert!(measured.allocations > 0);
            assert_eq!(growing[0], 42);
            assert_eq!(&growing[1..], encoded);
        });
    }
    {
        scenario_runtime::check("peak_fits_preflight", || {
            let row = selection();
            let context = Context::Record {
                position: Position(7),
            };
            let budget = Delivery::encoding_memory_bytes(context, &row).unwrap();
            let (encoded, measured) =
                measure(|| Delivery::encode_borrowed(context, &row, budget).unwrap());
            assert_eq!(measured.allocations, 2, "{measured:?}");
            assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
            assert_eq!(
                measured.peak,
                harmos_contract::Message::encoded_len(&row) + encoded.capacity()
            );
            assert_eq!(
                encoded,
                harmos_contract::Message::encode_to_vec(&Delivery::new(context, row).unwrap())
            );
        });
    }
    {
        scenario_runtime::check("native_decode_peak", || {
            let row = selection();
            let encoded = row.encode_to_vec();
            let (budget, inspected) = measure(|| Selection::decoding_memory(&encoded).unwrap());
            assert_eq!(inspected.allocations, 0, "{inspected:?}");
            let (decoded, measured) = measure(|| Selection::decode(&encoded).unwrap());
            assert_eq!(decoded, row);
            assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
            assert_eq!(decoded.rows.capacity(), decoded.rows.len());
            assert_eq!(
                decoded.rows[0].row.row.row.row.row.payload.capacity(),
                32_768
            );
        });
    }
    {
        scenario_runtime::check("boxed_decode_peak", || {
            for choice in [
                BoxedChoice::Selected(Box::new(nested())),
                BoxedChoice::Named {
                    row: Box::new(Leaf {
                        payload: vec![7; 4096],
                    }),
                },
            ] {
                let row = BoxedSelection {
                    row: Box::new(nested()),
                    choice,
                };
                let encoded = row.encode_to_vec();
                let (budget, inspected) =
                    measure(|| BoxedSelection::decoding_memory(&encoded).unwrap());
                assert_eq!(inspected.allocations, 0, "{inspected:?}");
                let (decoded, measured) = measure(|| BoxedSelection::decode(&encoded).unwrap());
                assert!(decoded == row);
                assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
                assert_eq!(decoded.row.row.row.row.row.row.payload.capacity(), 32_768);
            }
        });
    }
    {
        scenario_runtime::check("boxed_allocates_after", || {
            use harmos_contract::schema::Described;
            let tag = |fields: &[harmos_contract::schema::graph::Field<_>], name: &str| {
                fields.iter().find(|field| field.name == name).unwrap().tag
            };
            // A Varint Where The Boxed Leaf Declares Bytes
            let mut row = Vec::new();
            prost::encoding::uint64::encode(tag(Leaf::FIELDS, "payload"), &1, &mut row);
            let mut bytes = Vec::new();
            prost::encoding::bytes::encode(tag(BoxedLeaf::FIELDS, "row"), &row, &mut bytes);

            let (refused, inspected) = measure(|| BoxedLeaf::decoding_memory(&bytes));
            assert!(refused.is_err());
            assert_eq!(inspected.allocations, 0, "{inspected:?}");
            let (refused, decoded) = measure(|| BoxedLeaf::decode(&bytes));
            assert!(refused.is_err());
            assert_eq!(decoded.allocations, 0, "{decoded:?}");
        });
    }
    {
        scenario_runtime::check("preflight_never_allocate", || {
            for bytes in [&[0xff][..], &[0][..], &[10, 255][..]] {
                let (refused, measured) = measure(|| Selection::decoding_memory(bytes));
                assert!(refused.is_err());
                assert_eq!(measured.allocations, 0, "{measured:?}");
            }
        });
    }
    {
        scenario_runtime::check("strings_fit_bound", || {
            let before = Strings {
                label: "a".repeat(513),
                optional: Some("b".repeat(257)),
                labels: vec!["c".repeat(129), String::new()],
                counts: vec![1, 128],
            };
            let after = Strings {
                label: "d".repeat(1025),
                optional: Some(String::new()),
                labels: vec!["e".repeat(65)],
                counts: vec![u32::MAX],
            };
            let mut encoded = before.encode_to_vec();
            encoded.extend(after.encode_to_vec());
            let (budget, inspected) = measure(|| Strings::decoding_memory(&encoded).unwrap());
            assert_eq!(inspected.allocations, 0, "{inspected:?}");
            let (decoded, measured) = measure(|| Strings::decode(&encoded).unwrap());
            assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
            assert_eq!(decoded.label, after.label);
            assert_eq!(decoded.optional, after.optional);
            assert_eq!(decoded.labels, [before.labels, after.labels].concat());
            assert_eq!(decoded.counts, [1, 128, u32::MAX]);
            assert_eq!(decoded.labels.capacity(), 3);
            assert_eq!(decoded.counts.capacity(), 3);
        });
    }
    {
        scenario_runtime::check("core_header_expansion", || {
            use harmos_contract::MemorySize;
            use harmos_contract::operations::host::EgressRequest;
            let encoded = [0x22, 0].repeat(4096);
            let (budget, inspected) = measure(|| EgressRequest::decoding_memory(&encoded).unwrap());
            assert_eq!(inspected.allocations, 0, "{inspected:?}");
            let (decoded, measured) = measure(|| EgressRequest::decode(&encoded).unwrap());
            assert_eq!(decoded.headers.len(), 4096);
            assert_eq!(decoded.headers.capacity(), 4096);
            assert_eq!(measured.allocations, 1, "{measured:?}");
            assert!(decoded.memory_size() > encoded.len());
            assert!(decoded.memory_size() <= budget);
            assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
        });
    }
    {
        scenario_runtime::check("headers_refuse_container", || {
            use harmos_contract::operations::host::EgressRequest;
            let mut encoded = [0x22, 0].repeat(4096);
            encoded.extend_from_slice(&[0x22, 3, 0x0a, 1, 255]);
            let (refused, inspected) = measure(|| EgressRequest::decoding_memory(&encoded));
            assert!(refused.is_err());
            assert_eq!(inspected.allocations, 0, "{inspected:?}");
            let (refused, decoded) = measure(|| EgressRequest::decode(&encoded));
            assert!(refused.is_err());
            assert_eq!(decoded.allocations, 0, "{decoded:?}");
        });
    }
    {
        scenario_runtime::check("listener_decode_borrows", || {
            let row = Leaf {
                payload: vec![9; 32768],
            };
            let context = Context::Record {
                position: Position(7),
            };
            let encoding = Delivery::encoding_memory_bytes(context, &row).unwrap();
            let encoded = Delivery::encode_borrowed(context, &row, encoding).unwrap();
            let (bound, inspected) = measure(|| {
                <Delivery<Leaf> as harmos_contract::Message>::decoding_memory(&encoded).unwrap()
            });
            assert_eq!(inspected.allocations, 0, "{inspected:?}");
            let (decoded, allocated) =
                measure(|| <Delivery<Leaf> as harmos_contract::Message>::decode(&encoded).unwrap());
            assert_eq!(decoded.row(), &row);
            assert_eq!(allocated.allocations, 1, "{allocated:?}");
            assert!(allocated.peak <= bound, "{allocated:?}, bound {bound}");
        });
    }
    {
        scenario_runtime::check("context_refuses_allocations", || {
            // Record Context With An Invalid Zero Journal Position
            let bytes = [0, 0, 0];
            let (refused, inspected) =
                measure(|| <Delivery<Leaf> as harmos_contract::Message>::decoding_memory(&bytes));
            assert!(refused.is_err());
            assert_eq!(inspected.allocations, 0, "{inspected:?}");
            let (refused, decoded) =
                measure(|| <Delivery<Leaf> as harmos_contract::Message>::decode(&bytes));
            assert!(refused.is_err());
            assert_eq!(decoded.allocations, 0, "{decoded:?}");
        });
    }
}
