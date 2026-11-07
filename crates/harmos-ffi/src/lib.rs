/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Marshaling And Ownership Only; The Sidecar Kit Owns Execution And Transport.

#![allow(unsafe_code)]

/* === Modules === */

mod error;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::ffi::{CStr, c_char, c_void};
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};

// Third Party Imports

// Local Imports
use error::Error;
use harmos_domain::Failure;
use harmos_domain::operations::{FailureKind, OperationPolicy, RetryPolicy};
use harmos_sidecar::kit::dynamic::{Context, DynamicPlugin};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type HrmRun = unsafe extern "C" fn(*mut c_void, *mut HrmContext, HrmSlice, *mut HrmBuffer, *mut HrmError) -> i32;

/* === Structs === */

pub struct HrmPlugin {
    plugin: Option<DynamicPlugin>,
    capacity: NonZeroUsize,
}

pub struct HrmContext(Context);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HrmSlice {
    pub data: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Default)]
pub struct HrmBuffer {
    pub data: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Default)]
pub struct HrmError {
    pub code: i32,
    pub message: HrmBuffer,
}

#[repr(C)]
pub struct HrmPolicy {
    pub timeout_ms: u64,
    pub attempt_timeout_ms: u64,
    pub cancellation_timeout_ms: u64,
}

pub struct ArrowArray {
    _private: [u8; 0],
}

pub struct ArrowSchema {
    _private: [u8; 0],
}

struct Output {
    response: HrmBuffer,
    error: HrmError,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl HrmBuffer {
    fn new(bytes: Vec<u8>) -> Self {
        let bytes = bytes.into_boxed_slice();
        let len = bytes.len();
        Self { data: Box::into_raw(bytes).cast(), len }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: Both Values Come From This ABI's Allocator And Are Consumed Once.
        unsafe {
            hrm_buffer_free(&mut self.response);
            hrm_error_free(&mut self.error);
        }
    }
}

/* === Public Functions === */

// Returns The Rust Authoring Field Tag, Or Zero For Invalid UTF-8, Empty Input Or Panic.
// # Safety
// The Name Slice Is Borrowed And Valid For The Duration Of This Call.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_schema_field_tag(name: HrmSlice) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        let name = std::str::from_utf8(unsafe { bytes(name) }.ok()?).ok()?;
        if name.is_empty() {
            return None;
        }
        Some(harmos_domain::schema::field_tag(name))
    }))
    .ok()
    .flatten()
    .unwrap_or(0)
}

// # Safety
// `id` Is NUL-Terminated; `error`, If Present, Points To An Empty Writable Error.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_plugin_new(id: *const c_char, capacity: usize, error: *mut HrmError) -> *mut HrmPlugin {
    let mut result = std::ptr::null_mut();
    let status = boundary(error, || {
        let capacity = NonZeroUsize::new(capacity).ok_or(Error::InvalidInput("capacity must be positive"))?;
        let plugin = DynamicPlugin::new(unsafe { text(id)? }, capacity)?;
        result = Box::into_raw(Box::new(HrmPlugin { plugin: Some(plugin), capacity }));
        Ok(())
    });
    if status == 0 { result } else { std::ptr::null_mut() }
}

// # Safety
// Handles Must Be Live And Exclusive; Strings And Slices Are Borrowed For This Call.
// Callback And User State Remain Valid Until Serve Settles; Callback Must Never Unwind.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_plugin_operation(
    plugin: *mut HrmPlugin, id: *const c_char, schema: HrmSlice, policy: HrmPolicy, run: HrmRun, user: *mut c_void, error: *mut HrmError,
) -> i32 {
    boundary(error, || {
        let plugin = unsafe { plugin.as_mut() }
            .ok_or(Error::InvalidInput("plugin is null"))?
            .plugin
            .as_mut()
            .ok_or(Error::InvalidInput("plugin already served"))?;
        let schema = serde_json::from_slice(unsafe { bytes(schema)? })?;
        let policy = OperationPolicy {
            timeout: std::time::Duration::from_millis(policy.timeout_ms),
            attempt_timeout: std::time::Duration::from_millis(policy.attempt_timeout_ms),
            cancellation_timeout: std::time::Duration::from_millis(policy.cancellation_timeout_ms),
            retries: RetryPolicy::Never,
        };
        let user = user as usize;
        plugin.operation(unsafe { text(id)? }, schema, policy, move |context, request| {
            let request = serde_json::to_vec(&request).map_err(failure)?;
            let mut context = HrmContext(context);
            let mut output = Output {
                response: HrmBuffer::default(),
                error: HrmError::default(),
            };
            // SAFETY: The Author Keeps Callback State Alive; All Borrows End On Return.
            let status = unsafe {
                run(
                    user as *mut c_void,
                    &mut context,
                    HrmSlice {
                        data: request.as_ptr(),
                        len: request.len(),
                    },
                    &mut output.response,
                    &mut output.error,
                )
            };
            if status != 0 {
                let message = unsafe {
                    bytes(HrmSlice {
                        data: output.error.message.data,
                        len: output.error.message.len,
                    })
                }
                .map_err(failure)?;
                return Err(failure(String::from_utf8_lossy(message)));
            }
            let response = unsafe {
                bytes(HrmSlice {
                    data: output.response.data,
                    len: output.response.len,
                })
            }
            .map_err(failure)?;
            serde_json::from_slice(response).map_err(failure)
        })?;
        Ok(())
    })
}

// # Safety
// `plugin` Is Live And Exclusive; `name` Is NUL-Terminated; `error` Is Empty.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_plugin_stream(plugin: *mut HrmPlugin, name: *const c_char, window: usize, error: *mut HrmError) -> i32 {
    boundary(error, || {
        let plugin = unsafe { plugin.as_mut() }
            .ok_or(Error::InvalidInput("plugin is null"))?
            .plugin
            .as_mut()
            .ok_or(Error::InvalidInput("plugin already served"))?;
        let window = NonZeroUsize::new(window).ok_or(Error::InvalidInput("stream window must be positive"))?;
        plugin.stream(unsafe { text(name)? }, window)?;
        Ok(())
    })
}

// # Safety
// `plugin` Is Live And Exclusive Until Return; `error` Is Empty. Serve Runs Once.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_plugin_serve(plugin: *mut HrmPlugin, error: *mut HrmError) -> i32 {
    boundary(error, || {
        let plugin = unsafe { plugin.as_mut() }.ok_or(Error::InvalidInput("plugin is null"))?;
        let serving = plugin.plugin.take().ok_or(Error::InvalidInput("plugin already served"))?;
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(plugin.capacity.get()).build()?;
        runtime.block_on(serving.serve())?;
        Ok(())
    })
}

// # Safety
// `plugin` Is Null Or A Unique Handle Returned By New. Serve And All Callbacks Ended.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_plugin_free(plugin: *mut HrmPlugin) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !plugin.is_null() {
            unsafe {
                drop(Box::from_raw(plugin));
            }
        }
    }));
}

// Returns 1 For Cancellation, 0 Otherwise, Or -1 For An Invalid Context Or Panic.
// # Safety
// `context` Is Borrowed From The Currently Running Callback.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_context_cancelled(context: *const HrmContext) -> i32 {
    catch_unwind(AssertUnwindSafe(|| unsafe { context.as_ref() }.map_or(-1, |context| i32::from(context.0.cancelled())))).unwrap_or(-1)
}

// Blocks Until This Callback's Cancellation Is Requested.
// # Safety
// Context Is Borrowed From This Callback On Its Rust-Owned Thread; Error Is Empty.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_context_wait_cancelled(context: *const HrmContext, error: *mut HrmError) -> i32 {
    boundary(error, || {
        let context = unsafe { context.as_ref() }.ok_or(Error::InvalidInput("context is null"))?;
        context.0.wait_cancelled();
        Ok(())
    })
}

// # Safety
// Context Is Borrowed From This Callback; Id Is NUL-Terminated; Input Is Borrowed.
// Response And Error Are Empty Writable Values And Must Be Freed With This ABI.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_context_operation(
    context: *const HrmContext, id: *const c_char, request: HrmSlice, response: *mut HrmBuffer, error: *mut HrmError,
) -> i32 {
    boundary(error, || {
        let context = unsafe { context.as_ref() }.ok_or(Error::InvalidInput("context is null"))?;
        let response = unsafe { response.as_mut() }.ok_or(Error::InvalidInput("response is null"))?;
        let request = serde_json::from_slice(unsafe { bytes(request)? })?;
        let result = context.0.operation(unsafe { text(id)? }, &request).map_err(Error::Operation)?;
        *response = HrmBuffer::new(serde_json::to_vec(&result)?);
        Ok(())
    })
}

// Takes Ownership Of Both C Data Structures After Non-Null Argument Validation,
// Including On Import Or Publication Failure. Their Release Pointers Become Null.
// # Safety
// Context Is Borrowed From This Callback. Array And Schema Are Valid, Exclusive
// Arrow C Data Interface Structures With Live Buffers And Non-Unwinding Releases.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_context_publish_arrow(
    context: *const HrmContext, stream: *const c_char, array: *mut ArrowArray, schema: *mut ArrowSchema, error: *mut HrmError,
) -> i32 {
    boundary(error, || {
        let context = unsafe { context.as_ref() }.ok_or(Error::InvalidInput("context is null"))?;
        let stream = unsafe { text(stream)? };
        if array.is_null() || schema.is_null() {
            return Err(Error::InvalidInput("array or schema is null"));
        }
        // SAFETY: C Data Interface Move Replaces The Foreign Structs With Empty Ones.
        let array = unsafe { arrow_array::ffi::FFI_ArrowArray::from_raw(array.cast()) };
        let schema = unsafe { arrow_schema::ffi::FFI_ArrowSchema::from_raw(schema.cast()) };
        // SAFETY: The Caller Supplies A Valid C Data Array Whose Buffers Outlive Its Release.
        let data = unsafe { arrow_array::ffi::from_ffi(array, &schema) }?;
        if !matches!(data.data_type(), arrow_schema::DataType::Struct(_)) {
            return Err(Error::InvalidInput("batch must be a struct array"));
        }
        let batch = arrow_array::RecordBatch::from(arrow_array::StructArray::from(data));
        context.0.publish_arrow(stream, batch).map_err(Error::Operation)?;
        Ok(())
    })
}

// # Safety
// Input Is Borrowed; Output And Error Are Empty Writable Values.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_buffer_copy(input: HrmSlice, output: *mut HrmBuffer, error: *mut HrmError) -> i32 {
    boundary(error, || {
        let output = unsafe { output.as_mut() }.ok_or(Error::InvalidInput("output is null"))?;
        *output = HrmBuffer::new(unsafe { bytes(input)? }.to_vec());
        Ok(())
    })
}

// # Safety
// Buffer Is Null, Empty Or Uniquely Owns An Allocation Returned By This ABI.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_buffer_free(buffer: *mut HrmBuffer) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(buffer) = unsafe { buffer.as_mut() } {
            let owned = std::mem::take(buffer);
            if !owned.data.is_null() {
                unsafe {
                    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(owned.data, owned.len)));
                }
            }
        }
    }));
}

// # Safety
// Error Is Null, Empty Or Uniquely Owns A Message Returned By This ABI.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn hrm_error_free(error: *mut HrmError) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(error) = unsafe { error.as_mut() } {
            unsafe {
                hrm_buffer_free(&mut error.message);
            }
            error.code = 0;
        }
    }));
}

/* === Private Functions === */

fn boundary(error: *mut HrmError, run: impl FnOnce() -> Result<(), Error>) -> i32 {
    let outcome = catch_unwind(AssertUnwindSafe(|| run().map_err(|error| (error.code(), error.to_string()))));
    let (code, message) = match outcome {
        Ok(Ok(())) => return 0,
        Ok(Err(error)) => error,
        Err(payload) => {
            // A Panic Payload May Itself Have A Panicking Destructor.
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                std::mem::forget(payload);
            }
            (Error::Panic.code(), Error::Panic.to_string())
        }
    };
    if let Some(error) = unsafe { error.as_mut() } {
        *error = HrmError {
            code,
            message: HrmBuffer::new(message.into_bytes()),
        };
    }
    code
}

unsafe fn text<'a>(text: *const c_char) -> Result<&'a str, Error> {
    if text.is_null() {
        return Err(Error::InvalidInput("string is null"));
    }
    Ok(unsafe { CStr::from_ptr(text) }.to_str()?)
}

unsafe fn bytes<'a>(value: HrmSlice) -> Result<&'a [u8], Error> {
    if value.len == 0 {
        return Ok(&[]);
    }
    if value.data.is_null() || value.len > isize::MAX as usize {
        return Err(Error::InvalidInput("invalid byte slice"));
    }
    Ok(unsafe { std::slice::from_raw_parts(value.data, value.len) })
}

fn failure(error: impl std::fmt::Display) -> Failure {
    Failure::new(FailureKind::Permanent, error.to_string())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_slice_returns_an_owned_error_and_allows_the_next_call() {
        let mut error = HrmError::default();
        let mut output = HrmBuffer::default();
        let invalid = HrmSlice { data: std::ptr::null(), len: 1 };
        // SAFETY: The Slots Are Empty And Owned; Null Input Must Be Rejected Before Reading.
        let status = unsafe { hrm_buffer_copy(invalid, &mut output, &mut error) };
        assert_ne!(status, 0);
        assert_eq!(status, error.code);
        assert!(output.data.is_null());
        // SAFETY: This ABI Allocated The Error Message And This Test Owns It.
        let message = unsafe {
            bytes(HrmSlice {
                data: error.message.data,
                len: error.message.len,
            })
        }
        .unwrap();
        assert!(!message.is_empty());
        unsafe { hrm_error_free(&mut error) };
        assert!(error.message.data.is_null());

        let payload = b"copied after refusal";
        let input = HrmSlice {
            data: payload.as_ptr(),
            len: payload.len(),
        };
        // SAFETY: The Payload Is Live And The Output And Error Slots Are Empty.
        let status = unsafe { hrm_buffer_copy(input, &mut output, &mut error) };
        assert_eq!(status, 0);
        let actual = unsafe { bytes(HrmSlice { data: output.data, len: output.len }) }.unwrap();
        assert_eq!(actual, payload);
        unsafe { hrm_buffer_free(&mut output) };
    }

    #[test]
    fn panic_is_contained_at_the_abi_boundary() {
        let mut error = HrmError::default();
        extern "C" fn probe(error: *mut HrmError) -> i32 {
            boundary(error, || panic!("injected ABI panic"))
        }
        let status = probe(&mut error);
        assert_ne!(status, 0);
        assert_ne!(error.code, 0);
        // SAFETY: The Boundary Allocated This Error And This Test Is Its Sole Owner.
        unsafe {
            hrm_error_free(&mut error);
        }
        assert!(error.message.data.is_null());

        // Enter A Production Export With A Panicking Rust Destructor.
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        struct PanicOnDrop(Arc<AtomicBool>);
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
                panic!("injected destructor panic");
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let bomb = PanicOnDrop(Arc::clone(&dropped));
        // SAFETY: The Literal And Error Slot Remain Valid; This Test Owns The Returned Handle.
        let native = unsafe { hrm_plugin_new(c"panic".as_ptr(), 1, &mut error) };
        assert!(!native.is_null());
        let schema = serde_json::from_value(serde_json::json!({
            "root": 0, "definitions": [[
                {"name": "request", "tag": 1, "kind": {"type": "message", "definition": 1}, "cardinality": "required"},
                {"name": "response", "tag": 2, "kind": {"type": "message", "definition": 2}, "cardinality": "required"}
            ], [], []]
        }))
        .unwrap();
        let policy = OperationPolicy {
            timeout: std::time::Duration::from_secs(5),
            attempt_timeout: std::time::Duration::from_secs(5),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        };
        // SAFETY: The Unserved Handle Is Unique And Remains Owned Until The Export Below.
        unsafe { &mut *native }
            .plugin
            .as_mut()
            .unwrap()
            .operation("panic.drop", schema, policy, move |_, _| {
                std::hint::black_box(&bomb);
                Ok(serde_json::json!({}))
            })
            .unwrap();
        // SAFETY: No Callbacks Started; This Consumes The Original Allocation Exactly Once.
        unsafe {
            hrm_plugin_free(native);
        }
        assert!(dropped.load(Ordering::SeqCst));
    }
}
