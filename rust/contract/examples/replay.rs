//! Writes the native contract's response to every recorded request, one JSON line each, so
//! the WebAssembly build can be checked against it byte for byte
//! (`planner-wasm/tests/node-replay.mjs`).
//!
//! Usage: cargo run --release -p contract --example replay -- <contract.jsonl> <out.jsonl>

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = std::fs::read_to_string(&args[1]).expect("read the fixture");
    let mut out =
        std::io::BufWriter::new(std::fs::File::create(&args[2]).expect("create the output"));

    for line in input.lines() {
        let case: serde_json::Value = serde_json::from_str(line).expect("a JSON line");
        let result = match case["op"].as_str().unwrap() {
            "plan" => contract::plan_json(case["request"].as_str().unwrap()),
            "normalize" => contract::normalize_json(case["request"].as_str().unwrap()),
            _ => continue,
        };
        let record = match result {
            Ok(response) => {
                serde_json::json!({ "name": case["name"], "op": case["op"], "status": 0, "output": response })
            }
            Err(thrown) => {
                serde_json::json!({ "name": case["name"], "op": case["op"], "status": 1, "output": thrown.to_string() })
            }
        };
        writeln!(out, "{record}").expect("write");
    }
}
