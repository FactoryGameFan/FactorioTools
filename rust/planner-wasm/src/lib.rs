//! The planner as a WebAssembly module (#138). It replaced the .NET build in `src/BrowserWasm`
//! (removed in stage 13b) and its two `[JSExport]` methods, `Plan` and `Normalize`.
//!
//! The exports use plain numbers, so no binding generator is needed. `js/planner.js` wraps
//! them in the same `plan(requestJson)` / `normalize(requestJson)` functions as
//! `src/vue/src/lib/wasmPlanner.ts`. A call goes:
//!
//! 1. `input_buffer(len)` makes room for the request's UTF-8 bytes and returns where to
//!    copy them in the module's memory;
//! 2. `plan()` or `normalize()` runs the contract and returns a status: 0 for a response
//!    (including an error envelope), 1 when the C# would have thrown (the output is then the
//!    exception's message), 2 when the request is not valid UTF-8;
//! 3. `output_ptr()` and `output_len()` locate the UTF-8 result, which stays valid until the
//!    next call.
//!
//! The buffers are owned here, so no raw pointer is ever read. Nothing uses the browser's
//! main thread, so the module runs the same in a Web Worker. A panic traps (WebAssembly has
//! no unwinding), and the wrapper starts a fresh instance after one.

use std::cell::RefCell;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

const OK: u32 = 0;
const THROWN: u32 = 1;
const INVALID_UTF8: u32 = 2;

/// Makes room for `len` request bytes and returns their address in linear memory.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn input_buffer(len: usize) -> *mut u8 {
    INPUT.with_borrow_mut(|input| {
        input.clear();
        input.resize(len, 0);
        input.as_mut_ptr()
    })
}

fn run(call: fn(&str) -> Result<String, contract::Thrown>) -> u32 {
    let (status, output) = INPUT.with_borrow(|input| match std::str::from_utf8(input) {
        Ok(request) => match call(request) {
            Ok(response) => (OK, response),
            Err(thrown) => (THROWN, thrown.to_string()),
        },
        Err(_) => (INVALID_UTF8, "The request is not valid UTF-8.".to_string()),
    });
    OUTPUT.set(output.into_bytes());
    status
}

/// `Interop.Plan`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn plan() -> u32 {
    run(contract::plan_json)
}

/// `Interop.Normalize`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn normalize() -> u32 {
    run(contract::normalize_json)
}

/// The address of the last result.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const u8 {
    OUTPUT.with_borrow(|output| output.as_ptr())
}

/// The length in bytes of the last result.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn output_len() -> usize {
    OUTPUT.with_borrow(Vec::len)
}
