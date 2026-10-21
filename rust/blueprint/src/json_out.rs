//! A small JSON writer that produces the same text as .NET's `System.Text.Json` with its
//! default settings: compact output, strings escaped by `JavaScriptEncoder.Default`, and
//! numbers in .NET's shortest round-trip format. The blueprint and contract JSON are
//! written with it so their text matches the C#'s, not only their values.

/// Builds one JSON document. Commas are placed automatically.
#[derive(Default)]
pub struct JsonWriter {
    out: String,
    /// One entry per open object or array: whether it already holds an item.
    has_items: Vec<bool>,
    /// A property name was just written, so the next value needs no comma.
    after_name: bool,
}

impl JsonWriter {
    pub fn new() -> Self {
        JsonWriter::default()
    }

    pub fn finish(self) -> String {
        self.out
    }

    fn before_value(&mut self) {
        if self.after_name {
            self.after_name = false;
            return;
        }
        if let Some(has_items) = self.has_items.last_mut() {
            if *has_items {
                self.out.push(',');
            }
            *has_items = true;
        }
    }

    pub fn begin_object(&mut self) -> &mut Self {
        self.before_value();
        self.out.push('{');
        self.has_items.push(false);
        self
    }

    pub fn end_object(&mut self) -> &mut Self {
        self.has_items.pop();
        self.out.push('}');
        self
    }

    pub fn begin_array(&mut self) -> &mut Self {
        self.before_value();
        self.out.push('[');
        self.has_items.push(false);
        self
    }

    pub fn end_array(&mut self) -> &mut Self {
        self.has_items.pop();
        self.out.push(']');
        self
    }

    /// A property name inside an object.
    pub fn name(&mut self, name: &str) -> &mut Self {
        self.before_value();
        write_string(&mut self.out, name);
        self.out.push(':');
        self.after_name = true;
        self
    }

    pub fn string(&mut self, value: &str) -> &mut Self {
        self.before_value();
        write_string(&mut self.out, value);
        self
    }

    pub fn string_or_null(&mut self, value: Option<&str>) -> &mut Self {
        match value {
            Some(value) => self.string(value),
            None => self.null(),
        }
    }

    pub fn null(&mut self) -> &mut Self {
        self.before_value();
        self.out.push_str("null");
        self
    }

    pub fn bool(&mut self, value: bool) -> &mut Self {
        self.before_value();
        self.out.push_str(if value { "true" } else { "false" });
        self
    }

    pub fn int(&mut self, value: i64) -> &mut Self {
        self.before_value();
        self.out.push_str(&value.to_string());
        self
    }

    pub fn uint(&mut self, value: u64) -> &mut Self {
        self.before_value();
        self.out.push_str(&value.to_string());
        self
    }

    pub fn f32(&mut self, value: f32) -> &mut Self {
        self.before_value();
        self.out.push_str(&format_f32(value));
        self
    }

    pub fn f64(&mut self, value: f64) -> &mut Self {
        self.before_value();
        self.out.push_str(&format_f64(value));
        self
    }
}

/// Writes `value` as a JSON string the way `JavaScriptEncoder.Default` escapes it: only
/// Basic Latin passes through, minus control characters and the HTML-sensitive
/// `"` `&` `'` `+` `<` `>` and `` ` ``, which become `\uXXXX` (upper-case hex). The backslash
/// and the usual control characters use their short escapes. Characters outside Basic Latin
/// are written as UTF-16 code units, so one outside the BMP becomes a surrogate pair.
pub fn write_string(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '"' | '&' | '\'' | '+' | '<' | '>' | '`' => push_unicode_escape(out, c as u16),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => push_unicode_escape(out, c as u16),
            c if c.is_ascii() => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    push_unicode_escape(out, *unit);
                }
            }
        }
    }
    out.push('"');
}

fn push_unicode_escape(out: &mut String, unit: u16) {
    out.push_str(&format!("\\u{unit:04X}"));
}

/// `float.ToString("R")`, which `Utf8JsonWriter` uses for `float`: the shortest text that
/// reads back to the same value, in scientific notation when the exponent is below -4 or
/// at least 7.
pub fn format_f32(value: f32) -> String {
    format_shortest(value.is_sign_negative(), &format!("{:e}", value.abs()), 7)
}

/// `double.ToString("R")`: as [`format_f32`], with scientific notation from exponent 15.
pub fn format_f64(value: f64) -> String {
    format_shortest(value.is_sign_negative(), &format!("{:e}", value.abs()), 15)
}

/// Formats Rust's shortest `{:e}` text (`d.ddde±x`) the way .NET's general format does.
fn format_shortest(negative: bool, scientific: &str, precision: i32) -> String {
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("`{:e}` always has an exponent");
    let exponent: i32 = exponent.parse().expect("an integer exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if negative { "-" } else { "" };

    if digits == "0" {
        return format!("{sign}0");
    }

    if exponent < -5 + 1 || exponent >= precision {
        // d.dddE+XX, with at least two exponent digits.
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        return format!(
            "{sign}{first}{fraction}E{exponent_sign}{:02}",
            exponent.abs()
        );
    }

    let point = exponent + 1;
    if point <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!(
            "{sign}{digits}{}",
            "0".repeat(point as usize - digits.len())
        )
    } else {
        let (whole, fraction) = digits.split_at(point as usize);
        format!("{sign}{whole}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_like_dotnet() {
        assert_eq!(format_f64(9.0), "9");
        assert_eq!(format_f64(7.5), "7.5");
        assert_eq!(format_f64(10.25), "10.25");
        assert_eq!(format_f64(-61.0), "-61");
        assert_eq!(format_f64(0.0001), "0.0001");
        assert_eq!(format_f64(0.00001), "1E-05");
        assert_eq!(format_f64(123456789012345.0), "123456789012345");
        assert_eq!(format_f64(1e15), "1E+15");
        assert_eq!(format_f64(1.5e20), "1.5E+20");
        assert_eq!(format_f64(0.1), "0.1");
        assert_eq!(format_f64(-0.0), "-0");
        assert_eq!(format_f64(0.0), "0");
    }

    #[test]
    fn floats_like_dotnet() {
        assert_eq!(format_f32(39.5), "39.5");
        assert_eq!(format_f32(8.0), "8");
        assert_eq!(format_f32(1234567.0), "1234567");
        assert_eq!(format_f32(1e7), "1E+07");
        assert_eq!(format_f32(0.1), "0.1");
    }

    #[test]
    fn strings_like_the_default_encoder() {
        let mut out = String::new();
        write_string(&mut out, "heat-pipe <&'+>\u{e9}\"\\\n\u{1}`");
        assert_eq!(
            out,
            "\"heat-pipe \\u003C\\u0026\\u0027\\u002B\\u003E\\u00E9\\u0022\\\\\\n\\u0001\\u0060\""
        );
        let mut out = String::new();
        write_string(&mut out, "\u{1F600}");
        assert_eq!(out, "\"\\uD83D\\uDE00\"");
    }

    #[test]
    fn commas_and_nesting() {
        let mut w = JsonWriter::new();
        w.begin_object();
        w.name("a").int(1);
        w.name("b").begin_array().int(1).int(2).end_array();
        w.name("c").begin_object().end_object();
        w.end_object();
        assert_eq!(w.finish(), r#"{"a":1,"b":[1,2],"c":{}}"#);
    }
}
