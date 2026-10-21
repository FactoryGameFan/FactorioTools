// Runs every recorded contract request through the WebAssembly build (via js/planner.js)
// and checks each result is byte-identical to the native Rust contract's, then times plan()
// on the 61 small-list requests with default options.
//
// Usage (from rust/):
//   cargo build --release -p planner-wasm --target wasm32-unknown-unknown
//   cargo run --release -p contract --example replay -- contract/fixtures/contract.jsonl <native.jsonl>
//   node planner-wasm/tests/node-replay.mjs contract/fixtures/contract.jsonl <native.jsonl> \
//     target/wasm32-unknown-unknown/release/planner_wasm.wasm

import { readFileSync } from 'node:fs';
import { init, normalize, plan } from '../js/planner.js';

const [fixturePath, nativePath, wasmPath] = process.argv.slice(2);
await init(readFileSync(wasmPath));

const native = new Map(
  readFileSync(nativePath, 'utf8').trim().split('\n').map((line) => {
    const r = JSON.parse(line);
    return [`${r.op}:${r.name}`, r];
  }),
);

let checked = 0;
const smallDefaults = [];
for (const line of readFileSync(fixturePath, 'utf8').trim().split('\n')) {
  const c = JSON.parse(line);
  if (c.op !== 'plan' && c.op !== 'normalize') continue;
  const expected = native.get(`${c.op}:${c.name}`);
  let status = 0;
  let output;
  try {
    output = await (c.op === 'plan' ? plan(c.request) : normalize(c.request));
  } catch (error) {
    status = 1;
    output = error.message;
  }
  if (status !== expected.status || output !== expected.output) {
    console.error(`MISMATCH ${c.op} ${c.name}: status ${status} vs ${expected.status}`);
    process.exit(1);
  }
  checked++;
  if (c.op === 'plan' && /^small-\d{4}_default$/.test(c.name)) smallDefaults.push(c.request);
}
console.log(`${checked} requests: the WebAssembly build matches the native contract byte for byte`);

const times = [];
for (let pass = 0; pass < 3; pass++) {
  const start = performance.now();
  for (const request of smallDefaults) await plan(request);
  times.push(performance.now() - start);
}
console.log(`plan() on ${smallDefaults.length} small-list requests (default options): ${times.map((t) => t.toFixed(0)).join(', ')} ms per pass`);
