//! `Quality` and `Qualities`, from `src/FactorioTools/OilField/Quality.cs`.

/// A Factorio 2.0 quality tier. The value is the quality bonus level that scales
/// quality-affected stats; the engine skips a hidden level 4, so legendary is level 5.
///
/// A newtype rather than a Rust enum, because a C# enum holds any integer: `(Quality)42` is a
/// valid value, and `Qualities.ToBlueprintString` maps it to "normal" (see `QualitiesTest`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Quality(pub i32);

impl Quality {
    pub const NORMAL: Quality = Quality(0);
    pub const UNCOMMON: Quality = Quality(1);
    pub const RARE: Quality = Quality(2);
    pub const EPIC: Quality = Quality(3);
    pub const LEGENDARY: Quality = Quality(5);

    /// The five named tiers, in declaration order (`Enum.GetValues<Quality>()`).
    pub const ALL: [Quality; 5] = [
        Quality::NORMAL,
        Quality::UNCOMMON,
        Quality::RARE,
        Quality::EPIC,
        Quality::LEGENDARY,
    ];

    /// `(int)quality`: the bonus level.
    pub const fn level(self) -> i32 {
        self.0
    }

    /// The C# member name (`Enum.ToString()`), or `None` for a value the enum does not name.
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0 => Some("Normal"),
            1 => Some("Uncommon"),
            2 => Some("Rare"),
            3 => Some("Epic"),
            5 => Some("Legendary"),
            _ => None,
        }
    }

    /// `Qualities.ToBlueprintString`: the lowercase name used in blueprint JSON. Any value the
    /// switch does not name, including undefined ones, falls through to "normal".
    pub const fn to_blueprint_string(self) -> &'static str {
        match self.0 {
            1 => "uncommon",
            2 => "rare",
            3 => "epic",
            5 => "legendary",
            _ => "normal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // QualityTest.ToBlueprintString_ReturnsLowercaseName and
    // QualitiesTest.ToBlueprintString_ReturnsLowercaseName.
    #[test]
    fn to_blueprint_string_returns_lowercase_name() {
        for (quality, expected) in [
            (Quality::NORMAL, "normal"),
            (Quality::UNCOMMON, "uncommon"),
            (Quality::RARE, "rare"),
            (Quality::EPIC, "epic"),
            (Quality::LEGENDARY, "legendary"),
        ] {
            assert_eq!(quality.to_blueprint_string(), expected);
        }
    }

    // QualitiesTest.ToBlueprintString_FallsBackToNormal_ForUnknownValue.
    #[test]
    fn to_blueprint_string_falls_back_to_normal_for_unknown_value() {
        assert_eq!(Quality(42).to_blueprint_string(), "normal");
    }

    // QualityTest.EnumValues_AreQualityBonusLevels and
    // QualitiesTest.QualityEnum_EncodesBonusLevel_WithHiddenLevelFourSkipped.
    #[test]
    fn values_are_quality_bonus_levels() {
        assert_eq!(Quality::NORMAL.level(), 0);
        assert_eq!(Quality::UNCOMMON.level(), 1);
        assert_eq!(Quality::RARE.level(), 2);
        assert_eq!(Quality::EPIC.level(), 3);
        assert_eq!(Quality::LEGENDARY.level(), 5);
    }
}
