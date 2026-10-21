// The oil-field planner: the Rust port (rust/planner-wasm) compiled to WebAssembly.
// `npm run build-wasm` builds it and copies planner.js and planner_wasm.wasm into
// ./planner-wasm/, where Vite gives the .wasm a content-hashed name under /assets/.
//
// Planning runs in a Web Worker (plannerWorker.ts), so the page stays responsive while a
// big field is planned. If the worker cannot start - it fails to construct, reports an
// error, or does not answer the first request within WORKER_START_TIMEOUT_MS - this module
// falls back for good to running the same planner on the main thread, which is how the
// .NET build always ran. (The .NET runtime deadlocked when booted in a worker; the Rust
// module has no runtime and does not.) Add `?planner=main-thread` to the page URL to force
// the fallback, for testing.
//
// plan() and normalize() resolve to the response JSON, which may be an error envelope, and
// reject when the planner threw - the same contract the .NET build had.

import type { PlannerOp, PlannerReply, PlannerRequest } from "./plannerWorker"

const WORKER_START_TIMEOUT_MS = 15_000

type Pending = { resolve: (response: string) => void; reject: (error: Error) => void }

let worker: Worker | null = null
let workerReady: Promise<boolean> | null = null
let useMainThread = false
let nextId = 1
const pending = new Map<number, Pending>()

/** The worker went away while a request was waiting; the request reruns on the main thread. */
class WorkerLostError extends Error {
  constructor() {
    super("The planner worker stopped.")
  }
}

function forcedMainThread(): boolean {
  try {
    return new URLSearchParams(globalThis.location?.search ?? "").get("planner") === "main-thread"
  } catch {
    return false
  }
}

function fallBack(reason: string) {
  if (!useMainThread) {
    console.warn(`Planner worker unavailable (${reason}); planning on the main thread.`)
  }
  useMainThread = true
  worker?.terminate()
  worker = null
  // Anything still waiting on the worker reruns on the main thread.
  const waiting = [...pending.values()]
  pending.clear()
  for (const p of waiting) {
    p.reject(new WorkerLostError())
  }
}

function send(op: PlannerOp, request: string): Promise<string> {
  return new Promise((resolve, reject) => {
    if (!worker) {
      reject(new WorkerLostError())
      return
    }
    const id = nextId++
    pending.set(id, { resolve, reject })
    worker.postMessage({ id, op, request } satisfies PlannerRequest)
  })
}

function startWorker(): Promise<boolean> {
  if (workerReady) {
    return workerReady
  }
  workerReady = (async () => {
    if (forcedMainThread()) {
      fallBack("forced by ?planner=main-thread")
      return false
    }
    try {
      worker = new Worker(new URL("./plannerWorker.ts", import.meta.url), { type: "module" })
    } catch (error) {
      fallBack(`could not construct it: ${String(error)}`)
      return false
    }
    worker.addEventListener("message", (event: MessageEvent<PlannerReply>) => {
      const reply = event.data
      const p = pending.get(reply.id)
      if (!p) {
        return
      }
      pending.delete(reply.id)
      if (reply.ok) {
        p.resolve(reply.response)
      } else {
        p.reject(new Error(reply.message))
      }
    })
    worker.addEventListener("error", (event) => fallBack(`error: ${event.message || "unknown"}`))
    worker.addEventListener("messageerror", () => fallBack("a message could not be read"))

    let timer: ReturnType<typeof setTimeout> | undefined
    const timeout = new Promise<never>((_, reject) => {
      timer = setTimeout(
        () => reject(new Error(`no answer within ${WORKER_START_TIMEOUT_MS} ms`)),
        WORKER_START_TIMEOUT_MS,
      )
    })
    try {
      await Promise.race([send("init", ""), timeout])
      return true
    } catch (error) {
      fallBack(`it did not start: ${error instanceof Error ? error.message : String(error)}`)
      return false
    } finally {
      clearTimeout(timer)
    }
  })()
  return workerReady
}

async function onMainThread(op: "plan" | "normalize", request: string): Promise<string> {
  const planner = await import("./planner-wasm/planner.js")
  return op === "plan" ? planner.plan(request) : planner.normalize(request)
}

async function run(op: "plan" | "normalize", request: string): Promise<string> {
  if (!useMainThread && (await startWorker())) {
    try {
      return await send(op, request)
    } catch (error) {
      // A planner error is the caller's to handle. Only a lost worker reruns here.
      if (!(error instanceof WorkerLostError)) {
        throw error
      }
    }
  }
  return onMainThread(op, request)
}

export function plan(requestJson: string): Promise<string> {
  return run("plan", requestJson)
}

export function normalize(requestJson: string): Promise<string> {
  return run("normalize", requestJson)
}
