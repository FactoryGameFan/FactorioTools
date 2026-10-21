//! Factorio blueprint strings, from `src/FactorioTools.Serialization` (#138).
//!
//! A blueprint string is a version character `'0'`, then base64 of zlib-compressed JSON.
//! Reading ports `ParseBlueprint` (`OilField/Steps/ParseBlueprint.cs`); writing ports
//! `GridToBlueprintString` ([`write`]), with .NET's JSON text formats in [`json_out`].
//!
//! The planner crate stays free of serde, so the JSON is read into this crate's own
//! [`RawBlueprintRoot`] model first and then converted to the planner's `data` types. The raw
//! model keeps every entity's direction as the integer in the JSON. The planner's `Direction`
//! holds only the four values a pumpjack can face, which is all the planner reads: like the
//! C#, only pumpjack directions are converted to internal ones, and another entity's direction
//! carries over only when it is one of those four values.

pub mod json_out;
pub mod write;

pub use write::{grid_to_blueprint_string, serialize_blueprint};

use std::io::Read;

use base64::Engine;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use planner::data::{
    Blueprint, Direction, Entity, EntityItems, Icon, Position, SignalId, entity_names,
};
use planner::error::{FactorioToolsError, FtResult};
use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

/// The blueprint version at which Factorio widened directions from 8-way to 16-way:
/// `GridToBlueprintString.FormatVersion(2, 0, 0, 0)`.
const FIRST_SIXTEEN_WAY_VERSION: u64 = 562_949_953_421_312;

/// `BlueprintRoot`, as JSON.
#[derive(Debug, Default, Deserialize)]
pub struct RawBlueprintRoot {
    #[serde(default)]
    pub blueprint: Option<RawBlueprint>,
    #[serde(default)]
    pub blueprint_book: Option<IgnoredAny>,
}

/// `Blueprint`, as JSON.
#[derive(Debug, Default, Deserialize)]
pub struct RawBlueprint {
    #[serde(default)]
    pub icons: Option<Vec<RawIcon>>,
    #[serde(default)]
    pub entities: Option<Vec<RawEntity>>,
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub version: u64,
}

/// `Icon`, as JSON.
#[derive(Debug, Default, Deserialize)]
pub struct RawIcon {
    #[serde(default)]
    pub signal: Option<RawSignalId>,
    #[serde(default)]
    pub index: i32,
}

/// `SignalID`, as JSON.
#[derive(Debug, Default, Deserialize)]
pub struct RawSignalId {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "type")]
    pub signal_type: Option<String>,
}

/// `Entity`, as JSON. `direction` is the raw integer, before any conversion.
#[derive(Debug, Default, Deserialize)]
pub struct RawEntity {
    #[serde(default)]
    pub entity_number: i32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub position: Option<RawPosition>,
    #[serde(default)]
    pub direction: Option<i32>,
    #[serde(default)]
    pub mirror: Option<bool>,
    #[serde(default)]
    pub items: Option<RawItems>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub neighbours: Option<Vec<i32>>,
}

/// `Position`, as JSON. The numbers are kept as their text and parsed straight to `f32`.
#[derive(Debug, Default)]
pub struct RawPosition {
    pub x: f32,
    pub y: f32,
}

impl<'de> Deserialize<'de> for RawPosition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Text<'a> {
            #[serde(borrow, default)]
            x: Option<&'a RawValue>,
            #[serde(borrow, default)]
            y: Option<&'a RawValue>,
        }

        fn parse<E: de::Error>(value: Option<&RawValue>) -> Result<f32, E> {
            match value {
                None => Ok(0.0),
                Some(value) => value
                    .get()
                    .parse::<f32>()
                    .map_err(|_| E::custom("a position is not a number")),
            }
        }

        let text = Text::deserialize(deserializer)?;
        Ok(RawPosition {
            x: parse(text.x)?,
            y: parse(text.y)?,
        })
    }
}

/// `Entity.items`, read the way `EntityItemsConverter.Read` reads it: a Factorio 1.1 object
/// of item name to count becomes [`EntityItems::Counts`], in the order the names first appear
/// (a repeated name keeps its first place and takes the last count, as a `Dictionary` does).
/// Anything else, such as the Factorio 2.0 array, is skipped.
#[derive(Debug)]
pub struct RawItems(pub Option<Vec<(String, i32)>>);

impl<'de> Deserialize<'de> for RawItems {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ItemsVisitor;

        impl<'de> Visitor<'de> for ItemsVisitor {
            type Value = RawItems;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an items object or array")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RawItems, A::Error> {
                let mut counts: Vec<(String, i32)> = Vec::new();
                while let Some((name, count)) = map.next_entry::<String, i32>()? {
                    match counts.iter_mut().find(|(n, _)| *n == name) {
                        Some(existing) => existing.1 = count,
                        None => counts.push((name, count)),
                    }
                }
                Ok(RawItems(Some(counts)))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<RawItems, A::Error> {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(RawItems(None))
            }

            fn visit_bool<E: de::Error>(self, _: bool) -> Result<RawItems, E> {
                Ok(RawItems(None))
            }

            fn visit_i64<E: de::Error>(self, _: i64) -> Result<RawItems, E> {
                Ok(RawItems(None))
            }

            fn visit_u64<E: de::Error>(self, _: u64) -> Result<RawItems, E> {
                Ok(RawItems(None))
            }

            fn visit_f64<E: de::Error>(self, _: f64) -> Result<RawItems, E> {
                Ok(RawItems(None))
            }

            fn visit_str<E: de::Error>(self, _: &str) -> Result<RawItems, E> {
                Ok(RawItems(None))
            }
        }

        deserializer.deserialize_any(ItemsVisitor)
    }
}

/// `ParseBlueprint.ToInternalDirection`: a blueprint direction as the internal four-way
/// `Direction`. Blueprints from Factorio 2.0 on (and unstamped ones, version 0) use 16-way
/// directions, halved here; an odd value or a diagonal is rejected.
pub fn to_internal_direction(raw: i32, version: u64) -> FtResult<Direction> {
    let mut internal_value = raw;

    if (version == 0 || version >= FIRST_SIXTEEN_WAY_VERSION) && raw % 2 == 0 {
        internal_value = raw / 2;
    }

    Direction::from_value(internal_value).ok_or_else(|| {
        FactorioToolsError::with_bad_input(
            format!(
                "Blueprint direction {raw} is not one of the four directions a pumpjack can face."
            ),
            true,
        )
    })
}

/// `ParseBlueprint.ReadBlueprintFile`: the file's trimmed lines, without empty lines and
/// `#` comments.
pub fn read_blueprint_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    Ok(read_blueprint_lines(&text))
}

/// [`read_blueprint_file`] on text already read. `File.ReadAllLines` splits on `\r\n`, `\n`
/// and `\r`.
pub fn read_blueprint_lines(text: &str) -> Vec<String> {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .split("\r\n")
        .flat_map(|line| line.split(['\n', '\r']))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The "could not be fully decoded" error. The C# reports it for every failure while
/// decoding, since its `looksLikeJson` flag starts true and only a decoded JSON text that
/// does not start with `{` can turn it off.
///
/// `inner` is the wrapped exception's message, which the contract's error envelope lists
/// after this one. The base64 and zlib messages are .NET's own text. A JSON error carries
/// serde_json's description instead, since `System.Text.Json`'s wording (and its own nested
/// inner exception) cannot be reproduced in general.
fn not_fully_decoded(inner: impl Into<String>) -> FactorioToolsError {
    FactorioToolsError::with_bad_input(
        "Input blueprint string could not be fully decoded. Are you sure you copied the whole blueprint?",
        true,
    )
    .with_inner_message(inner)
}

/// `Convert.FromBase64String`'s `FormatException` message.
const BASE64_FORMAT_MESSAGE: &str = "The input is not a valid Base-64 string as it contains a non-base 64 character, more than two padding characters, or an illegal character among the padding characters.";

/// The `InvalidDataException` message .NET's `ZLibStream` gives for data zlib rejects.
const ZLIB_DATA_MESSAGE: &str =
    "The archive entry was compressed using an unsupported compression method.";

/// Decodes a blueprint string to its JSON text, as the C# does: base64 after the version
/// character (missing `=` padding added, whitespace ignored as `Convert.FromBase64String`
/// does), then zlib, then UTF-8 with a byte order mark removed.
pub fn decode_blueprint_json(blueprint_string: &str) -> FtResult<String> {
    let mut base64: String = blueprint_string[1..]
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
        .collect();
    let missing_padding = (4 - (base64.len() % 4)) % 4;
    for _ in 0..missing_padding {
        base64.push('=');
    }

    let engine = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_allow_trailing_bits(true)
            .with_decode_padding_mode(DecodePaddingMode::RequireCanonical),
    );
    let bytes = engine
        .decode(&base64)
        .map_err(|_| not_fully_decoded(BASE64_FORMAT_MESSAGE))?;

    let mut decompressed = Vec::new();
    flate2::read::ZlibDecoder::new(&bytes[..])
        .read_to_end(&mut decompressed)
        .map_err(|_| not_fully_decoded(ZLIB_DATA_MESSAGE))?;

    let text = String::from_utf8_lossy(&decompressed);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

/// Reads a blueprint string into the raw JSON model, with the C#'s error messages.
pub fn parse_raw(blueprint_string: &str) -> FtResult<RawBlueprintRoot> {
    if blueprint_string.is_empty() {
        return Err(FactorioToolsError::with_bad_input(
            "Input blueprint string is empty.",
            true,
        ));
    }

    if !blueprint_string.starts_with('0') {
        return Err(FactorioToolsError::with_bad_input(
            "Input blueprint does not have the expected version byte of '0' at the beginning.",
            true,
        ));
    }

    let json = decode_blueprint_json(blueprint_string)?;
    let root: Option<RawBlueprintRoot> =
        serde_json::from_str(&json).map_err(|e| not_fully_decoded(e.to_string()))?;

    root.ok_or_else(|| {
        FactorioToolsError::with_bad_input("The blueprint JSON deserialized as null.", true)
    })
}

/// `ParseBlueprint.Execute`: a blueprint string as the planner's `Blueprint`.
pub fn parse_blueprint(blueprint_string: &str) -> FtResult<Blueprint> {
    let root = parse_raw(blueprint_string)?;

    if root.blueprint_book.is_some() {
        return Err(FactorioToolsError::with_bad_input(
            "The blueprint provided contains a blueprint book, not an individual blueprint.",
            true,
        ));
    }

    let Some(raw) = root.blueprint else {
        return Err(FactorioToolsError::with_bad_input(
            "No blueprint was found in the deserialized JSON.",
            true,
        ));
    };

    let mut entities = Vec::new();
    for entity in raw.entities.unwrap_or_default() {
        let name = entity.name.unwrap_or_default();

        // Only pumpjack directions are converted, because only they are read downstream.
        let direction = match entity.direction {
            None => None,
            Some(raw_direction) if name == entity_names::vanilla::PUMPJACK => {
                Some(to_internal_direction(raw_direction, raw.version)?)
            }
            Some(raw_direction) => Direction::from_value(raw_direction),
        };

        let position = entity.position.unwrap_or_default();
        entities.push(Entity {
            entity_number: entity.entity_number,
            name,
            position: Position {
                x: position.x,
                y: position.y,
            },
            direction,
            mirror: entity.mirror,
            items: entity
                .items
                .and_then(|items| items.0)
                .map(EntityItems::Counts),
            quality: entity.quality,
            neighbours: entity.neighbours,
        });
    }

    let icons = raw
        .icons
        .unwrap_or_default()
        .into_iter()
        .map(|icon| {
            let signal = icon.signal.unwrap_or_default();
            Icon {
                signal: SignalId {
                    name: signal.name.unwrap_or_default(),
                    signal_type: signal.signal_type.unwrap_or_default(),
                },
                index: icon.index,
            }
        })
        .collect();

    Ok(Blueprint {
        icons,
        entities,
        item: raw.item.unwrap_or_default(),
        version: raw.version,
    })
}
