//! `System.Math` rounding and C# numeric conversions.

/// `Math.Round(x, 0)` (and `Math.Round(x)`): `MidpointRounding.ToEven`, so 0.5 rounds to 0
/// and 1.5 rounds to 2. Rust's `f64::round` rounds halves away from zero instead.
pub fn round_half_even(x: f64) -> f64 {
    x.round_ties_even()
}

/// `(int)Math.Round(x, 0)`.
pub fn round_to_i32(x: f64) -> i32 {
    truncate_to_i32(round_half_even(x))
}

/// `(int)x`: truncate toward zero. Only defined here for values that fit in an `int`, the
/// only ones the planner converts; .NET's result for anything else is not relied on.
pub fn truncate_to_i32(x: f64) -> i32 {
    debug_assert!(
        x > f64::from(i32::MIN) - 1.0 && x < f64::from(i32::MAX) + 1.0,
        "(int) conversion out of range: {x}"
    );
    x as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_round_to_even() {
        assert_eq!(round_half_even(0.5), 0.0);
        assert_eq!(round_half_even(1.5), 2.0);
        assert_eq!(round_half_even(-2.5), -2.0);
        assert_eq!(round_to_i32(2.5), 2);
        assert_eq!(truncate_to_i32(-1.9), -1);
    }
}
