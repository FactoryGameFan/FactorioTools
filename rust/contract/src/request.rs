//! Reading request JSON the way `ContractJson.Options` does in the C#: property names match
//! case-insensitively, a repeated property keeps its last value, numbers may also be given
//! as strings (`AllowReadingFromString`), enums are their names (any case) or integers
//! (`JsonStringEnumConverter`), and unknown properties are skipped.

use std::fmt;

use serde::Deserializer as _;
use serde::de::{self, MapAccess, Visitor};
use serde_json::value::RawValue;

use crate::Thrown;

/// A request's top level: an object's properties in document order, each kept as JSON
/// text, or `null`.
pub enum Root {
    Object(Vec<(String, Box<RawValue>)>),
    Null,
}

/// Reads the top level of `json`. Anything but an object or `null` is the C#'s
/// `JsonException` ("could not be converted"), as is malformed JSON.
pub fn read_root(json: &str, type_name: &str) -> Result<Root, Thrown> {
    struct RootVisitor;

    impl<'de> Visitor<'de> for RootVisitor {
        type Value = Option<Vec<(String, Box<RawValue>)>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a JSON object or null")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut properties = Vec::new();
            while let Some((name, value)) = map.next_entry::<String, Box<RawValue>>()? {
                properties.push((name, value));
            }
            Ok(Some(properties))
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    let mut deserializer = serde_json::Deserializer::from_str(json);
    let root = deserializer
        .deserialize_any(RootVisitor)
        .and_then(|root| deserializer.end().map(|_| root))
        .map_err(|e| {
            Thrown::json(format!(
                "The JSON value could not be converted to {type_name}. {e}"
            ))
        })?;

    Ok(match root {
        Some(properties) => Root::Object(properties),
        None => Root::Null,
    })
}

/// One property's value as the C# property type reads it.
pub struct Property<'a> {
    pub name: &'a str,
    pub raw: &'a str,
}

impl Property<'_> {
    fn error(&self, type_name: &str) -> Thrown {
        Thrown::json(format!(
            "The JSON value could not be converted to {type_name}. Path: $.{}",
            self.name
        ))
    }

    fn is_null(&self) -> bool {
        self.raw == "null"
    }

    /// `bool`: only `true` or `false`.
    pub fn bool(&self) -> Result<bool, Thrown> {
        match self.raw {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(self.error("System.Boolean")),
        }
    }

    /// `string?`.
    pub fn string(&self) -> Result<Option<String>, Thrown> {
        if self.is_null() {
            return Ok(None);
        }
        serde_json::from_str::<String>(self.raw)
            .map(Some)
            .map_err(|_| self.error("System.String"))
    }

    /// `int`: an integer token, or a string holding one.
    pub fn int(&self) -> Result<i32, Thrown> {
        read_int(self.raw).ok_or_else(|| self.error("System.Int32"))
    }

    /// `double`: a number token, or a string holding one. Named values such as `NaN` are not
    /// allowed (`AllowNamedFloatingPointLiterals` is off).
    pub fn double(&self) -> Result<f64, Thrown> {
        read_double(self.raw).ok_or_else(|| self.error("System.Double"))
    }

    /// `List<TEnum>?`, each item a name or an integer.
    pub fn enum_list(
        &self,
        names: &[(&str, i32)],
        type_name: &str,
    ) -> Result<Option<Vec<i32>>, Thrown> {
        if self.is_null() {
            return Ok(None);
        }
        let items: Vec<Box<RawValue>> =
            serde_json::from_str(self.raw).map_err(|_| self.error(type_name))?;
        items
            .iter()
            .map(|item| read_enum(item.get(), names).ok_or_else(|| self.error(type_name)))
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    /// An enum: a name (any case) or an integer.
    pub fn enum_value(&self, names: &[(&str, i32)], type_name: &str) -> Result<i32, Thrown> {
        read_enum(self.raw, names).ok_or_else(|| self.error(type_name))
    }

    /// `Dictionary<string, int>?`. A repeated key keeps its first place and its last value.
    pub fn int_dictionary(&self) -> Result<Option<Vec<(String, i32)>>, Thrown> {
        if self.is_null() {
            return Ok(None);
        }
        let type_name = "System.Collections.Generic.Dictionary`2[System.String,System.Int32]";
        let root = read_root(self.raw, type_name).map_err(|_| self.error(type_name))?;
        let Root::Object(entries) = root else {
            return Err(self.error(type_name));
        };
        let mut dictionary: Vec<(String, i32)> = Vec::new();
        for (key, value) in entries {
            let count = read_int(value.get()).ok_or_else(|| self.error("System.Int32"))?;
            match dictionary.iter_mut().find(|(k, _)| *k == key) {
                Some(existing) => existing.1 = count,
                None => dictionary.push((key, count)),
            }
        }
        Ok(Some(dictionary))
    }
}

/// An `int` from a number token or a string holding one, as `Utf8JsonReader.TryGetInt32`
/// and `AllowReadingFromString` read it: digits with an optional sign, nothing else.
fn read_int(raw: &str) -> Option<i32> {
    let text = if raw.starts_with('"') {
        serde_json::from_str::<String>(raw).ok()?
    } else {
        raw.to_string()
    };
    let digits = text.strip_prefix(['-', '+']).unwrap_or(&text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !raw.starts_with('"') && text.starts_with('+') {
        return None;
    }
    text.parse().ok()
}

/// A `double` from a number token or a string holding one.
fn read_double(raw: &str) -> Option<f64> {
    let text = if raw.starts_with('"') {
        serde_json::from_str::<String>(raw).ok()?
    } else {
        raw.to_string()
    };
    let valid = !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'));
    if !valid {
        return None;
    }
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// An enum value: one of `names` (compared ignoring case) or any integer token.
fn read_enum(raw: &str, names: &[(&str, i32)]) -> Option<i32> {
    if raw.starts_with('"') {
        let text: String = serde_json::from_str(raw).ok()?;
        return names
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&text))
            .map(|(_, value)| *value);
    }
    read_int(raw)
}
