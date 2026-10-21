// The oil-field planner (Rust, WebAssembly), with the same interface as
// src/vue/src/lib/wasmPlanner.ts: plan(requestJson) and normalize(requestJson) each take the
// request JSON and resolve to the response JSON (or to an error envelope, which is a normal
// response). A request the C# contract would have thrown on rejects with an Error.
//
// The module loads planner_wasm.wasm from next to this file on first use. Call init() first
// to load it from somewhere else (a URL, a Response, bytes, or a compiled WebAssembly.Module),
// which is also how to use it where fetch cannot read files, such as Node.
//
// Nothing here needs the main thread, so it works the same in a Web Worker.

let instancePromise = null;
let source = null;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Sets where the module comes from. Optional; the default is ./planner_wasm.wasm. */
export function init(moduleSource) {
  source = moduleSource;
  instancePromise = null;
  return load();
}

async function instantiate() {
  const from = source ?? new URL('./planner_wasm.wasm', import.meta.url);
  if (from instanceof WebAssembly.Module) {
    return WebAssembly.instantiate(from, {});
  }
  if (from instanceof ArrayBuffer || ArrayBuffer.isView(from)) {
    return (await WebAssembly.instantiate(from, {})).instance;
  }
  const response = from instanceof Response ? from : await fetch(from);
  if (WebAssembly.instantiateStreaming && response.headers.get('content-type') === 'application/wasm') {
    return (await WebAssembly.instantiateStreaming(response, {})).instance;
  }
  return (await WebAssembly.instantiate(await response.arrayBuffer(), {})).instance;
}

function load() {
  if (!instancePromise) {
    instancePromise = instantiate().catch((error) => {
      instancePromise = null;
      throw error;
    });
  }
  return instancePromise;
}

function call(exports, name, requestJson) {
  const bytes = encoder.encode(requestJson);
  const pointer = exports.input_buffer(bytes.length);
  new Uint8Array(exports.memory.buffer, pointer, bytes.length).set(bytes);
  const status = exports[name]();
  // Copy the result out before anything else can grow (and move) the memory.
  const text = decoder.decode(
    new Uint8Array(exports.memory.buffer, exports.output_ptr(), exports.output_len()).slice(),
  );
  if (status !== 0) {
    throw new Error(text);
  }
  return text;
}

async function run(name, requestJson) {
  const instance = await load();
  try {
    return call(instance.exports, name, requestJson);
  } catch (error) {
    // A trap (a Rust panic) leaves the instance unusable, so start fresh next time.
    if (error instanceof WebAssembly.RuntimeError) {
      instancePromise = null;
    }
    throw error;
  }
}

/** Plans an oil field. Resolves to the response JSON. */
export function plan(requestJson) {
  return run('plan', requestJson);
}

/** Normalizes a blueprint. Resolves to the response JSON. */
export function normalize(requestJson) {
  return run('normalize', requestJson);
}
