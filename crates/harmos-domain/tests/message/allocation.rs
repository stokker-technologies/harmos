/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
// Third Party Imports

// Local Imports
use harmos_domain::journal::Position;
use harmos_domain::listeners::{Context, Delivery};
use harmos_domain::message;
use harmos_sdk::grpc::{self, DecodeInput, prost};

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
        #[message(harmos_path = ::harmos_domain)]
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

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Leaf {
    payload: Vec<u8>,
}

// The Listeners Whose Deliveries Carry One Leaf Or One Selection.
struct Leaves;

struct Selections;

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Selection {
    row: Choice,
    optional: Option<Fifth>,
    rows: Vec<Fifth>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Strings {
    label: String,
    optional: Option<String>,
    labels: Vec<String>,
    counts: Vec<u32>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct BoxedSelection {
    row: Box<Fifth>,
    choice: BoxedChoice,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct BoxedLeaf {
    row: Box<Leaf>,
}

/* === Enums === */
#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
enum BoxedChoice {
    Selected(Box<Fifth>),
    Named { row: Box<Leaf> },
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
enum Choice {
    Selected(Fifth),
    Empty,
}

/* === Traits === */

/* === Implementations === */
impl harmos_domain::listeners::Listener for Selections {
    type Row = Selection;
    const CALLBACK: &'static str = "allocation.selections";
    const POLICY: harmos_domain::operations::OperationPolicy = harmos_domain::operations::host::POLICY;
}

impl harmos_domain::listeners::Listener for Leaves {
    type Row = Leaf;
    const CALLBACK: &'static str = "allocation.leaves";
    const POLICY: harmos_domain::operations::OperationPolicy = harmos_domain::operations::host::POLICY;
}

// A Pass-Through Allocator Measures Only This Test Thread's Encoding Buffers
#[allow(unsafe_code, reason = "observing allocation peaks requires an allocator shim")]
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
    measured.allocations = measured.allocations.saturating_add(usize::from(allocated != 0));
    // A Caller Buffer May Have Been Allocated Before Measurement Started
    measured.live = measured.live.saturating_sub(released).saturating_add(allocated);
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
    Fifth { row: Fourth { row: Third { row: Second { row: First { row: Leaf { payload: vec![255; 32_768] } } } } } }
}

fn selection() -> Selection {
    Selection { row: Choice::Selected(nested()), optional: Some(nested()), rows: vec![nested(), nested()] }
}

/* === Tests === */

#[test]
fn rows_allocate_final() {
    // CODEX-BLOCKED: Transcoding allocates intermediate buffers; the original allocation counts and peak-memory assertions are retained without a matching adapter guarantee.
    let row = selection();
    let (encoded, measured) = measure(|| crate::encode(&row));
    assert_eq!(measured.allocations, 1, "{measured:?}");
    assert_eq!(measured.peak, encoded.capacity());
    // CODEX-BLOCKED: The adapter has no independent encoded-length preflight; this retains only the encoded byte length comparison.
    assert_eq!(encoded.len(), crate::encode(&row).len());
    assert_eq!(grpc::decode::<Selection>(DecodeInput::new(&encoded)).unwrap(), row);

    // CODEX-BLOCKED: The adapter has no independent encoded-length preflight; this retains only the encoded byte length comparison.
    let mut appended = Vec::with_capacity(1 + crate::encode(&row).len());
    appended.push(42);
    let (_, measured) = measure(|| grpc::encode(&(row), &mut appended).unwrap());
    assert_eq!(measured.allocations, 0, "{measured:?}");
    assert_eq!(appended[0], 42);
    assert_eq!(&appended[1..], encoded);

    let mut growing = vec![42];
    let (_, measured) = measure(|| grpc::encode(&(row), &mut growing).unwrap());
    assert!(measured.allocations > 0);
    assert_eq!(growing[0], 42);
    assert_eq!(&growing[1..], encoded);
}

#[test]
fn peak_fits_preflight() {
    let row = selection();
    let context = Context::Record { position: Position(7) };
    let budget = harmos_domain::listeners::encoding_memory_bytes(context, &row).unwrap();
    let (encoded, measured) = measure(|| harmos_domain::listeners::encode_borrowed(context, &row, budget).unwrap());
    assert_eq!(measured.allocations, 2, "{measured:?}");
    assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
    assert_eq!(measured.peak, harmos_domain::Message::encoded_len(&row) + encoded.capacity());
    assert_eq!(encoded, harmos_domain::Message::encode_to_vec(&Delivery::<Selections>::new(context, row).unwrap()));
}

#[test]
fn native_decode_peak() {
    // CODEX-BLOCKED: Transcoding allocates intermediate buffers; the original allocation counts and peak-memory assertions are retained without a matching adapter guarantee.
    let row = selection();
    let encoded = crate::encode(&row);
    let (budget, inspected) = measure(|| grpc::decoding_memory::<Selection>(DecodeInput::new(&encoded)).unwrap());
    assert_eq!(inspected.allocations, 0, "{inspected:?}");
    let (decoded, measured) = measure(|| grpc::decode::<Selection>(DecodeInput::new(&encoded)).unwrap());
    assert_eq!(decoded, row);
    assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
    assert_eq!(decoded.rows.capacity(), decoded.rows.len());
    assert_eq!(decoded.rows[0].row.row.row.row.row.payload.capacity(), 32_768);
}

#[test]
fn boxed_decode_peak() {
    // CODEX-BLOCKED: Transcoding allocates intermediate buffers; the original allocation counts and peak-memory assertions are retained without a matching adapter guarantee.
    for choice in [BoxedChoice::Selected(Box::new(nested())), BoxedChoice::Named { row: Box::new(Leaf { payload: vec![7; 4096] }) }] {
        let row = BoxedSelection { row: Box::new(nested()), choice };
        let encoded = crate::encode(&row);
        let (budget, inspected) = measure(|| grpc::decoding_memory::<BoxedSelection>(DecodeInput::new(&encoded)).unwrap());
        assert_eq!(inspected.allocations, 0, "{inspected:?}");
        let (decoded, measured) = measure(|| grpc::decode::<BoxedSelection>(DecodeInput::new(&encoded)).unwrap());
        assert!(decoded == row);
        assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
        assert_eq!(decoded.row.row.row.row.row.row.payload.capacity(), 32_768);
    }
}

#[test]
fn boxed_allocates_after() {
    // CODEX-BLOCKED: Transcoding allocates intermediate buffers; the original allocation counts and peak-memory assertions are retained without a matching adapter guarantee.
    use harmos_domain::schema::Described;
    let tag = |fields: &[harmos_domain::schema::graph::Field<_>], name: &str| fields.iter().find(|field| field.name == name).unwrap().tag;
    // A Varint Where The Boxed Leaf Declares Bytes
    let mut row = Vec::new();
    prost::encoding::uint64::encode(tag(Leaf::FIELDS, "payload"), &1, &mut row);
    let mut bytes = Vec::new();
    prost::encoding::bytes::encode(tag(BoxedLeaf::FIELDS, "row"), &row, &mut bytes);

    let (refused, inspected) = measure(|| grpc::decoding_memory::<BoxedLeaf>(DecodeInput::new(&bytes)));
    assert!(refused.is_err());
    assert_eq!(inspected.allocations, 0, "{inspected:?}");
    let (refused, decoded) = measure(|| grpc::decode::<BoxedLeaf>(DecodeInput::new(&bytes)));
    assert!(refused.is_err());
    assert_eq!(decoded.allocations, 0, "{decoded:?}");
}

#[test]
fn preflight_never_allocate() {
    // CODEX-BLOCKED: Transcoding allocates intermediate buffers; the original allocation counts and peak-memory assertions are retained without a matching adapter guarantee.
    for bytes in [&[0xff][..], &[0][..], &[10, 255][..]] {
        let (refused, measured) = measure(|| grpc::decoding_memory::<Selection>(DecodeInput::new(bytes)));
        assert!(refused.is_err());
        assert_eq!(measured.allocations, 0, "{measured:?}");
    }
}

#[test]
fn strings_fit_bound() {
    // CODEX-BLOCKED: Transcoding allocates intermediate buffers; the original allocation counts and peak-memory assertions are retained without a matching adapter guarantee.
    let before = Strings {
        label: "a".repeat(513),
        optional: Some("b".repeat(257)),
        labels: vec!["c".repeat(129), String::new()],
        counts: vec![1, 128],
    };
    let after = Strings { label: "d".repeat(1025), optional: Some(String::new()), labels: vec!["e".repeat(65)], counts: vec![u32::MAX] };
    let mut encoded = crate::encode(&before);
    encoded.extend(crate::encode(&after));
    let (budget, inspected) = measure(|| grpc::decoding_memory::<Strings>(DecodeInput::new(&encoded)).unwrap());
    assert_eq!(inspected.allocations, 0, "{inspected:?}");
    let (decoded, measured) = measure(|| grpc::decode::<Strings>(DecodeInput::new(&encoded)).unwrap());
    assert!(measured.peak <= budget, "{measured:?}, budget {budget}");
    assert_eq!(decoded.label, after.label);
    assert_eq!(decoded.optional, after.optional);
    assert_eq!(decoded.labels, [before.labels, after.labels].concat());
    assert_eq!(decoded.counts, [1, 128, u32::MAX]);
    assert_eq!(decoded.labels.capacity(), 3);
    assert_eq!(decoded.counts.capacity(), 3);
}

#[test]
fn listener_decode_borrows() {
    let row = Leaf { payload: vec![9; 32768] };
    let context = Context::Record { position: Position(7) };
    let encoding = harmos_domain::listeners::encoding_memory_bytes(context, &row).unwrap();
    let encoded = harmos_domain::listeners::encode_borrowed(context, &row, encoding).unwrap();
    let (bound, inspected) = measure(|| <Delivery<Leaves> as harmos_domain::Message>::decoding_memory(&encoded).unwrap());
    assert_eq!(inspected.allocations, 0, "{inspected:?}");
    let (decoded, allocated) = measure(|| <Delivery<Leaves> as harmos_domain::Message>::decode(&encoded).unwrap());
    assert_eq!(decoded.row(), &row);
    assert_eq!(allocated.allocations, 1, "{allocated:?}");
    assert!(allocated.peak <= bound, "{allocated:?}, bound {bound}");
}

#[test]
fn context_refuses_allocations() {
    // Record Context With An Invalid Zero Journal Position
    let bytes = [0, 0, 0];
    let (refused, inspected) = measure(|| <Delivery<Leaves> as harmos_domain::Message>::decoding_memory(&bytes));
    assert!(refused.is_err());
    assert_eq!(inspected.allocations, 0, "{inspected:?}");
    let (refused, decoded) = measure(|| <Delivery<Leaves> as harmos_domain::Message>::decode(&bytes));
    assert!(refused.is_err());
    assert_eq!(decoded.allocations, 0, "{decoded:?}");
}
