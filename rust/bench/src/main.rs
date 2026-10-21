//! Times the planner on the test blueprint lists (#138), the way the site calls it: each
//! blueprint goes through [`contract::plan_json`] with the site's default options, as the
//! WebAssembly module does, but natively. Run it from `rust/`:
//!
//! ```text
//! cargo run --release -p bench -- [--list small|big|all] [--runs N] [--limit N]
//!                                 [--json <path>] [--baseline <path>]
//! ```
//!
//! Each field is planned `--runs` times (default 5) and its median time is kept. The report
//! gives each list's total and the slowest fields. `--json` saves every field's time with a
//! hash of its input and its response. `--baseline` compares this run with a saved one,
//! field by field, and says whether any response changed, which is what a change meant to
//! keep the output identical must show. Times depend on the machine and on what else it is
//! doing, so compare runs from the same machine only. Nothing here fails on timing; the
//! snapshot tests check the output.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use serde_json::{Value, json};

const USAGE: &str = "usage: bench [--list small|big|all] [--runs N] [--limit N] \
                     [--json <path>] [--baseline <path>]";

/// The format of the `--json` file. Bump it if a field changes meaning.
const FORMAT: u64 = 1;

struct Args {
    lists: Vec<&'static str>,
    runs: usize,
    limit: Option<usize>,
    json: Option<PathBuf>,
    baseline: Option<PathBuf>,
}

/// One field's result.
struct Field {
    list: &'static str,
    index: usize,
    pumpjacks: usize,
    median_ms: f64,
    min_ms: f64,
    input: u64,
    response: u64,
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let mut fields = Vec::new();
    for &list in &args.lists {
        let path = test_dir().join(format!("{list}-list.txt"));
        let blueprints = match blueprint::read_blueprint_file(&path) {
            Ok(blueprints) => blueprints,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        };
        let count = args
            .limit
            .map_or(blueprints.len(), |n| n.min(blueprints.len()));
        for (index, blueprint) in blueprints.iter().take(count).enumerate() {
            match time_field(list, index, blueprint, args.runs) {
                Ok(field) => fields.push(field),
                Err(message) => {
                    eprintln!("{list} #{index}: {message}");
                    return ExitCode::FAILURE;
                }
            }
            if (index + 1) % 100 == 0 {
                eprintln!("{list}: {} of {count} fields", index + 1);
            }
        }
    }

    println!(
        "{} run{} per field, median kept. Times are this machine's only.",
        args.runs,
        if args.runs == 1 { "" } else { "s" }
    );
    for &list in &args.lists {
        report_list(list, &fields);
    }

    if let Some(path) = &args.json {
        let text = serde_json::to_string_pretty(&to_json(&fields, args.runs))
            .expect("the results serialize");
        if let Err(e) = std::fs::write(path, text + "\n") {
            eprintln!("{}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("\nSaved {} fields to {}", fields.len(), path.display());
    }

    if let Some(path) = &args.baseline {
        match read_baseline(path) {
            Ok(baseline) => compare(&fields, &baseline, path),
            Err(message) => {
                eprintln!("{}: {message}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        lists: vec!["small"],
        runs: 5,
        limit: None,
        json: None,
        baseline: None,
    };
    let mut raw = std::env::args().skip(1);
    while let Some(arg) = raw.next() {
        let mut value = || raw.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--list" => {
                args.lists = match value()?.as_str() {
                    "small" => vec!["small"],
                    "big" => vec!["big"],
                    "all" => vec!["small", "big"],
                    other => return Err(format!("unknown list {other:?}")),
                }
            }
            "--runs" => args.runs = positive(&value()?)?,
            "--limit" => args.limit = Some(positive(&value()?)?),
            "--json" => args.json = Some(PathBuf::from(value()?)),
            "--baseline" => args.baseline = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(args)
}

fn positive(text: &str) -> Result<usize, String> {
    match text.parse() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("expected a whole number above 0, got {text:?}")),
    }
}

/// `testdata/oil-field`, where the blueprint lists live.
fn test_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/oil-field")
}

fn time_field(
    list: &'static str,
    index: usize,
    blueprint: &str,
    runs: usize,
) -> Result<Field, String> {
    let pumpjacks = blueprint::parse_blueprint(blueprint)
        .map_err(|e| format!("the blueprint does not parse: {e:?}"))?
        .entities
        .iter()
        .filter(|e| e.name == "pumpjack")
        .count();
    // Only the blueprint is set, so every option keeps its default, as on the site.
    let request = json!({ "blueprint": blueprint }).to_string();

    let mut times = Vec::with_capacity(runs);
    let mut response = None;
    for _ in 0..runs {
        let start = Instant::now();
        let output =
            contract::plan_json(&request).map_err(|e| format!("the planner threw: {e}"))?;
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        let hash = fnv1a(output.as_bytes());
        if *response.get_or_insert(hash) != hash {
            return Err("two runs gave different responses".to_string());
        }
    }
    times.sort_by(f64::total_cmp);

    Ok(Field {
        list,
        index,
        pumpjacks,
        median_ms: median(&times),
        min_ms: times[0],
        input: fnv1a(blueprint.as_bytes()),
        response: response.expect("runs is above 0"),
    })
}

/// FNV-1a, 64-bit: a fixed hash, so saved results compare across builds and Rust versions.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The median of sorted, non-empty times.
fn median(sorted: &[f64]) -> f64 {
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// The nearest-rank percentile of sorted, non-empty times.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn report_list(list: &str, fields: &[Field]) {
    let mut of_list: Vec<&Field> = fields.iter().filter(|f| f.list == list).collect();
    if of_list.is_empty() {
        return;
    }
    let mut times: Vec<f64> = of_list.iter().map(|f| f.median_ms).collect();
    times.sort_by(f64::total_cmp);
    let total: f64 = times.iter().sum();

    println!("\n{list} list: {} fields", of_list.len());
    println!(
        "  total {total:.1} ms   median {:.1} ms   p90 {:.1} ms   max {:.1} ms",
        median(&times),
        percentile(&times, 90.0),
        times[times.len() - 1]
    );
    of_list.sort_by(|a, b| b.median_ms.total_cmp(&a.median_ms));
    println!("  slowest:");
    for f in of_list.iter().take(5) {
        println!(
            "    #{:<5} {:>4} pumpjacks  {:>9.1} ms  (fastest run {:.1} ms)",
            f.index, f.pumpjacks, f.median_ms, f.min_ms
        );
    }
}

fn to_json(fields: &[Field], runs: usize) -> Value {
    json!({
        "format": FORMAT,
        "runs": runs,
        "fields": fields.iter().map(|f| json!({
            "list": f.list,
            "index": f.index,
            "pumpjacks": f.pumpjacks,
            "ms": f.median_ms,
            "min_ms": f.min_ms,
            "input": format!("{:016x}", f.input),
            "response": format!("{:016x}", f.response),
        })).collect::<Vec<_>>(),
    })
}

/// A saved field: its time, and its input and response hashes.
struct Saved {
    ms: f64,
    input: String,
    response: String,
}

fn read_baseline(path: &Path) -> Result<HashMap<(String, u64), Saved>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let root: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if root["format"].as_u64() != Some(FORMAT) {
        return Err(format!("not a format {FORMAT} bench file"));
    }
    let fields = root["fields"].as_array().ok_or("no fields array")?;
    let mut saved = HashMap::new();
    for f in fields {
        let (Some(list), Some(index), Some(ms), Some(input), Some(response)) = (
            f["list"].as_str(),
            f["index"].as_u64(),
            f["ms"].as_f64(),
            f["input"].as_str(),
            f["response"].as_str(),
        ) else {
            return Err(format!("a field is missing a value: {f}"));
        };
        saved.insert(
            (list.to_string(), index),
            Saved {
                ms,
                input: input.to_string(),
                response: response.to_string(),
            },
        );
    }
    Ok(saved)
}

fn compare(fields: &[Field], baseline: &HashMap<(String, u64), Saved>, path: &Path) {
    println!("\nCompared with {}:", path.display());
    let mut lists: Vec<&str> = fields.iter().map(|f| f.list).collect();
    lists.dedup();
    for list in lists {
        let (mut now, mut then, mut log_ratio, mut matched) = (0.0, 0.0, 0.0, 0usize);
        let (mut missing, mut other_input) = (0usize, 0usize);
        let mut changed = Vec::new();
        for f in fields.iter().filter(|f| f.list == list) {
            let Some(saved) = baseline.get(&(list.to_string(), f.index as u64)) else {
                missing += 1;
                continue;
            };
            if saved.input != format!("{:016x}", f.input) {
                other_input += 1;
                continue;
            }
            matched += 1;
            now += f.median_ms;
            then += saved.ms;
            log_ratio += (saved.ms / f.median_ms).ln();
            if saved.response != format!("{:016x}", f.response) {
                changed.push(f.index);
            }
        }
        println!("  {list} list: {matched} fields compared");
        if matched > 0 {
            println!(
                "    total {now:.1} ms, was {then:.1} ms: {:.2}x as fast; per-field geometric mean {:.2}x",
                then / now,
                (log_ratio / matched as f64).exp()
            );
            if changed.is_empty() {
                println!("    responses: all {matched} identical");
            } else {
                let shown: Vec<String> = changed.iter().take(10).map(|i| format!("#{i}")).collect();
                println!(
                    "    responses: {} changed ({}{})",
                    changed.len(),
                    shown.join(", "),
                    if changed.len() > 10 { ", ..." } else { "" }
                );
            }
        }
        if missing > 0 {
            println!("    {} no entry in the baseline", fields_phrase(missing));
        }
        if other_input > 0 {
            println!(
                "    {} a different blueprint than the baseline, so skipped",
                fields_phrase(other_input)
            );
        }
    }
}

/// "1 field has" or "3 fields have".
fn fields_phrase(n: usize) -> String {
    if n == 1 {
        "1 field has".to_string()
    } else {
        format!("{n} fields have")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_and_percentile() {
        assert_eq!(median(&[1.0, 2.0, 9.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 4.0, 9.0]), 3.0);
        let ten: Vec<f64> = (1..=10).map(f64::from).collect();
        assert_eq!(percentile(&ten, 90.0), 9.0);
        assert_eq!(percentile(&ten, 100.0), 10.0);
        assert_eq!(percentile(&[5.0], 90.0), 5.0);
    }

    #[test]
    fn fnv1a_is_the_published_hash() {
        // Test vectors from the FNV reference: the empty string and "a".
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
