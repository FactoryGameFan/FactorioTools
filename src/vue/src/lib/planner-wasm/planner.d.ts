// Types for planner.js, which `npm run build-wasm` copies here from
// rust/planner-wasm/js/planner.js along with planner_wasm.wasm. That wrapper is plain
// JavaScript; this file describes it so the TypeScript that imports it type-checks
// before the build has run.

/** Sets where the WebAssembly module comes from. Optional; the default is the
 * planner_wasm.wasm next to planner.js. Resolves once the module is loaded. */
export function init(
  moduleSource?: string | URL | Response | BufferSource | WebAssembly.Module,
): Promise<unknown>

/** Plans an oil field. Resolves to the response JSON, which may be an error envelope.
 * Rejects with an Error where the C# contract would have thrown. */
export function plan(requestJson: string): Promise<string>

/** Normalizes a blueprint. Resolves to the response JSON. */
export function normalize(requestJson: string): Promise<string>
