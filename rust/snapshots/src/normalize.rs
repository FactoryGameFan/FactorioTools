//! `NormalizeBlueprints` (`src/FactorioTools.Serialization/OilField/Steps/NormalizeBlueprints.cs`),
//! the C# CLI's `oil-field normalize`: rewrites `big-list.txt` and `small-list.txt` so every
//! blueprint is cleaned to its pumpjacks, none appears twice, and none is in both lists.
//!
//! Two things differ on purpose. First, the C# writes each blueprint as it re-serializes
//! it, and zlib builds compress the same text to different bytes: flate2's output differs
//! from the committed lines, and so does .NET 10's, so either tool written literally would
//! rewrite every blueprint line with no change to any blueprint. Here a blueprint's
//! identity is its normalized JSON: the text `SerializeBlueprint` compresses for the
//! cleaned blueprint. A line whose own compressed data already decodes to exactly that text
//! is kept as it is, and lines are compared by that text, so two copies of one blueprint
//! compressed differently are duplicates. Second, where the C# asks on the console whether
//! to keep a line that does not normalize, this fails unless told to keep such lines.

use std::collections::HashSet;
use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use planner::error::FactorioToolsError;
use planner::steps::clean_blueprint;

/// What a line of a list file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Content {
    /// An empty line or a `#` comment (`Valid` false in the C#).
    Text,
    /// A blueprint. `json` is its normalized JSON, the identity lines are compared by.
    /// `output` is the line to write: the trimmed line itself when it is already
    /// normalized, otherwise the blueprint re-serialized.
    Blueprint { json: String, output: String },
    /// A line that failed to normalize, kept with `--keep-invalid` (the C#'s "y" answer).
    Invalid,
}

/// One line, as `NormalizeFile` reads it.
#[derive(Clone, Debug)]
struct Line {
    original: String,
    index: usize,
    content: Content,
}

/// What two lines are compared by: the normalized JSON for a blueprint, the line itself
/// otherwise. In the C# both are the one output string, `Normalized ?? Original`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key<'a> {
    Json(&'a str),
    Raw(&'a str),
}

impl Line {
    /// `Valid`: a blueprint, or an invalid line the user chose to keep.
    fn valid(&self) -> bool {
        !matches!(self.content, Content::Text)
    }

    fn key(&self) -> Key<'_> {
        match &self.content {
            Content::Blueprint { json, .. } => Key::Json(json),
            Content::Text | Content::Invalid => Key::Raw(&self.original),
        }
    }

    /// `Normalized ?? Original`.
    fn output(&self) -> &str {
        match &self.content {
            Content::Blueprint { output, .. } => output,
            Content::Text | Content::Invalid => &self.original,
        }
    }
}

/// The lines of a file as `File.ReadLines` gives them: a UTF-8 byte order mark dropped,
/// split on `\r\n`, `\n` and `\r`, with no empty line after a final line break.
pub fn file_lines(text: &str) -> Vec<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines: Vec<&str> = text
        .split("\r\n")
        .flat_map(|line| line.split(['\n', '\r']))
        .collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// The decompressed bytes of a blueprint string, read strictly: canonical base64 with no
/// whitespace or missing padding, and one zlib stream that uses every byte. `None` when
/// the string is anything else, even if the lenient reader would accept it.
fn strict_payload(blueprint_string: &str) -> Option<Vec<u8>> {
    let compressed = STANDARD.decode(blueprint_string.strip_prefix('0')?).ok()?;
    let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
    let mut payload = Vec::new();
    decoder.read_to_end(&mut payload).ok()?;
    (decoder.total_in() == compressed.len() as u64).then_some(payload)
}

fn describe(error: &FactorioToolsError) -> String {
    let mut text = error.message.clone();
    for inner in &error.inner_messages {
        text.push_str(" ---> ");
        text.push_str(inner);
    }
    text
}

/// `TryNormalize`: `None` for an empty line or a comment, otherwise the blueprint's
/// normalized JSON and the line to write for it.
fn try_normalize(line: &str) -> Result<Option<(String, String)>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let parsed = blueprint::parse_blueprint(trimmed).map_err(|e| describe(&e))?;
    let clean = clean_blueprint::execute(parsed).map_err(|e| describe(&e))?;
    if clean.entities.is_empty() {
        return Err("There are no pumpjacks in the blueprint.".to_string());
    }

    let json = blueprint::write::output_blueprint_json(&clean, false);
    let output = if strict_payload(trimmed).as_deref() == Some(json.as_bytes()) {
        trimmed.to_string()
    } else {
        blueprint::write::encode_blueprint_json(&json)
    };
    Ok(Some((json, output)))
}

/// `NormalizeFile`. The C# asks on the console whether to keep a line that fails; this
/// fails instead, naming the file and line, unless `keep_invalid` keeps such lines as
/// they are.
fn normalize_file(name: &str, text: &str, keep_invalid: bool) -> Result<Vec<Line>, String> {
    let mut lines = Vec::new();
    for (index, original) in file_lines(text).into_iter().enumerate() {
        let content = match try_normalize(original) {
            Ok(None) => Content::Text,
            Ok(Some((json, output))) => Content::Blueprint { json, output },
            Err(_) if keep_invalid => Content::Invalid,
            Err(error) => {
                return Err(format!(
                    "{name} line {}: {error}\nFix or remove the line, or run with --keep-invalid to keep lines like it unchanged.",
                    index + 1
                ));
            }
        };
        lines.push(Line {
            original: original.to_string(),
            index,
            content,
        });
    }
    Ok(lines)
}

/// `NormalizeBlueprints.Execute`: the new text of the input file. The input's blueprints
/// stay in order, less those in the exclude file and repeats. With `allow_comments`,
/// comments and empty lines are kept, and the exclude file's are merged in by line number.
pub fn execute(
    input: (&str, &str),
    exclude: (&str, &str),
    allow_comments: bool,
    keep_invalid: bool,
) -> Result<String, String> {
    let exclude_lines = normalize_file(exclude.0, exclude.1, keep_invalid)?;
    let input_lines = normalize_file(input.0, input.1, keep_invalid)?;
    let excluded: HashSet<Key<'_>> = exclude_lines
        .iter()
        .filter(|line| line.valid())
        .map(Line::key)
        .collect();

    let mut ordered: Vec<&Line> = input_lines
        .iter()
        .chain(&exclude_lines)
        .filter(|line| allow_comments || line.valid())
        .filter(|line| !line.valid() || !excluded.contains(&line.key()))
        .collect();
    // `OrderBy` is stable, so on a tie the input's line comes first.
    ordered.sort_by_key(|line| line.index);

    let mut added = HashSet::new();
    let mut output = String::new();
    for line in ordered {
        if added.insert(line.key()) {
            output.push_str(line.output());
            output.push('\n');
        }
    }
    Ok(output)
}

/// The CLI's `oil-field normalize`: the big list less the small list's blueprints and its
/// comments, then the small list less the new big list's blueprints. So a blueprint in
/// both lists ends up in the small list only. Each list is a `(name, text)` pair, the name
/// used in error messages. Returns the new big and small list texts.
pub fn normalize_lists(
    big: (&str, &str),
    small: (&str, &str),
    keep_invalid: bool,
) -> Result<(String, String), String> {
    let new_big = execute(big, small, false, keep_invalid)?;
    let new_small = execute(small, (big.0, &new_big), true, keep_invalid)?;
    Ok((new_big, new_small))
}

/// The normalized JSON of every blueprint line in a list file, with its 1-based line
/// number, for tests that look for repeats without rewriting anything.
pub fn blueprint_keys(name: &str, text: &str) -> Result<Vec<(usize, String)>, String> {
    Ok(normalize_file(name, text, false)?
        .into_iter()
        .filter_map(|line| match line.content {
            Content::Blueprint { json, .. } => Some((line.index + 1, json)),
            Content::Text | Content::Invalid => None,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// The first blueprint of the committed small list, which is already normalized.
    fn first_small_blueprint() -> String {
        crate::small_list().swap_remove(0)
    }

    /// The same payload as `line`, compressed at a different level, so the string differs.
    fn recompressed(line: &str) -> String {
        let payload = strict_payload(line).expect("a strict blueprint string");
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&payload).unwrap();
        format!("0{}", STANDARD.encode(encoder.finish().unwrap()))
    }

    #[test]
    fn file_lines_matches_read_lines() {
        assert_eq!(file_lines(""), Vec::<&str>::new());
        assert_eq!(file_lines("a"), ["a"]);
        assert_eq!(file_lines("\u{feff}a\r\nb\rc\n"), ["a", "b", "c"]);
        assert_eq!(file_lines("a\n\n"), ["a", ""]);
    }

    #[test]
    fn a_normalized_line_is_kept_as_it_is() {
        let line = first_small_blueprint();
        let other = recompressed(&line);
        assert_ne!(line, other);
        for kept in [&line, &other] {
            let (json, output) = try_normalize(&format!("  {kept} ")).unwrap().unwrap();
            assert_eq!(&output, kept);
            assert_eq!(strict_payload(kept).unwrap(), json.as_bytes());
        }
    }

    #[test]
    fn differently_compressed_copies_are_duplicates() {
        let line = first_small_blueprint();
        let text = format!("{line}\n{}\n", recompressed(&line));
        let output = execute(("in", &text), ("ex", ""), false, false).unwrap();
        assert_eq!(output, format!("{line}\n"));
    }

    #[test]
    fn an_invalid_line_fails_unless_kept() {
        let text = "# comment\nnot a blueprint\n";
        let error = execute(("in.txt", text), ("ex", ""), true, false).unwrap_err();
        assert!(error.starts_with("in.txt line 2: "), "{error}");
        assert_eq!(
            execute(("in.txt", text), ("ex", ""), true, true).unwrap(),
            text
        );
    }
}
