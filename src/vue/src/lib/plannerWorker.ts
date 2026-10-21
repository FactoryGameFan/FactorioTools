// Runs the Rust planner (rust/planner-wasm) in a dedicated Web Worker, so planning does not
// freeze the page. wasmPlanner.ts starts this worker and talks to it with these messages.
//
// Requests: { id, op: "init" | "plan" | "normalize", request }. "init" loads the WebAssembly
// module and is how wasmPlanner.ts learns the worker works at all.
// Replies: { id, ok: true, response } or { id, ok: false, message } when the planner threw.

import { init, normalize, plan } from "./planner-wasm/planner.js"

export type PlannerOp = "init" | "plan" | "normalize"

export interface PlannerRequest {
  id: number
  op: PlannerOp
  request: string
}

export type PlannerReply =
  { id: number; ok: true; response: string } | { id: number; ok: false; message: string }

// The DOM typings describe `self` as a Window, whose postMessage needs a target origin. In a
// dedicated worker it takes the message alone.
const scope = self as unknown as {
  postMessage(message: PlannerReply): void
  addEventListener(type: "message", listener: (event: MessageEvent<PlannerRequest>) => void): void
}

async function handle(op: PlannerOp, request: string): Promise<string> {
  switch (op) {
    case "init":
      await init()
      return ""
    case "plan":
      return await plan(request)
    case "normalize":
      return await normalize(request)
  }
}

scope.addEventListener("message", async (event) => {
  const { id, op, request } = event.data
  let reply: PlannerReply
  try {
    reply = { id, ok: true, response: await handle(op, request) }
  } catch (error) {
    reply = { id, ok: false, message: error instanceof Error ? error.message : String(error) }
  }
  scope.postMessage(reply)
})
