//! Rewrites `big-list.txt` and `small-list.txt` the way the C# CLI's `oil-field normalize`
//! does: `cargo run --release -p snapshots --bin normalize-lists`. See
//! [`snapshots::normalize`] for what it changes and how it differs from the C#.
//!
//! `--keep-invalid` keeps a line that does not normalize, where the C# would ask.
//! `--dir <path>` works on the lists in another directory instead of the test directory.

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: normalize-lists [--keep-invalid] [--dir <path>]";

fn main() -> ExitCode {
    let mut keep_invalid = false;
    let mut dir = snapshots::oil_field_test_dir();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--keep-invalid" => keep_invalid = true,
            "--dir" => match args.next() {
                Some(path) => dir = PathBuf::from(path),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
            },
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }

    let big_path = dir.join("big-list.txt");
    let small_path = dir.join("small-list.txt");
    let read = |path: &PathBuf| {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    };
    let big = read(&big_path);
    let small = read(&small_path);

    let big_name = big_path.display().to_string();
    let small_name = small_path.display().to_string();
    let (new_big, new_small) = match snapshots::normalize::normalize_lists(
        (&big_name, &big),
        (&small_name, &small),
        keep_invalid,
    ) {
        Ok(lists) => lists,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    for (path, old, new) in [
        (&big_path, &big, &new_big),
        (&small_path, &small, &new_small),
    ] {
        let count = |text: &str| snapshots::normalize::file_lines(text).len();
        if old == new {
            println!("{}: unchanged, {} lines", path.display(), count(new));
        } else {
            std::fs::write(path, new).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            println!(
                "{}: rewritten, {} lines, was {}",
                path.display(),
                count(new),
                count(old)
            );
        }
    }

    ExitCode::SUCCESS
}
