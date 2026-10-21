//! `CollectionExtensions`, from `src/FactorioTools/CollectionExtensions.cs`.
//!
//! The C# replaced LINQ with these so the core could transpile to Lua. Most map straight onto
//! Rust iterators. The ones kept here are those whose behavior is easy to get subtly wrong:
//!
//! - `MaxBy`/`MinBy`/`Max`/`Min` compare with `Comparer<T>.Default` and only replace on a
//!   strictly greater (or smaller) key, so the **first** of several equal keys wins. Rust's
//!   `Iterator::max_by_key` returns the *last* maximum, so it must not be used.
//! - `Single`/`First` throw `FactorioToolsException` with set messages.
//!
//! The context-bound helpers (`ToDictionary`, `ToSet`, `ToReadOnlySet`) need `Context`,
//! which comes with stage 3.

use dotnet_compat::NetCompare;

use crate::Location;
use crate::containers::LocationSet;
use crate::error::{FactorioToolsError, FtResult};

/// `MaxBy`: the first item with the largest key, or `None` when empty (the C#'s `default`).
pub fn max_by<T, K: NetCompare>(
    items: impl IntoIterator<Item = T>,
    mut key: impl FnMut(&T) -> K,
) -> Option<T> {
    let mut best: Option<(K, T)> = None;
    for item in items {
        let k = key(&item);
        match &best {
            Some((best_key, _)) if k.net_cmp(best_key).is_le() => {}
            _ => best = Some((k, item)),
        }
    }
    best.map(|(_, item)| item)
}

/// `MinBy`: the first item with the smallest key, or `None` when empty.
pub fn min_by<T, K: NetCompare>(
    items: impl IntoIterator<Item = T>,
    mut key: impl FnMut(&T) -> K,
) -> Option<T> {
    let mut best: Option<(K, T)> = None;
    for item in items {
        let k = key(&item);
        match &best {
            Some((best_key, _)) if k.net_cmp(best_key).is_ge() => {}
            _ => best = Some((k, item)),
        }
    }
    best.map(|(_, item)| item)
}

/// `Max(selector)`: the largest value, or `None` when empty. The C# returns `default`
/// (0 for numbers) for an empty source; callers that rely on that use `.unwrap_or(0)`.
pub fn max<K: NetCompare + Copy>(items: impl IntoIterator<Item = K>) -> Option<K> {
    max_by(items, |k| *k)
}

/// `Min(selector)`: the smallest value, or `None` when empty.
pub fn min<K: NetCompare + Copy>(items: impl IntoIterator<Item = K>) -> Option<K> {
    min_by(items, |k| *k)
}

/// `Single()`.
pub fn single<T>(items: impl IntoIterator<Item = T>) -> FtResult<T> {
    let mut found = None;
    for item in items {
        if found.is_some() {
            return Err(FactorioToolsError::new(
                "Only one item should exist in the source.",
            ));
        }
        found = Some(item);
    }
    found.ok_or_else(|| FactorioToolsError::new("An item should exist in the source."))
}

/// `Single(predicate)`.
pub fn single_where<T>(
    items: impl IntoIterator<Item = T>,
    mut predicate: impl FnMut(&T) -> bool,
) -> FtResult<T> {
    let mut found = None;
    for item in items {
        if predicate(&item) {
            if found.is_some() {
                return Err(FactorioToolsError::new(
                    "Only one item should have matched the predicate.",
                ));
            }
            found = Some(item);
        }
    }
    found.ok_or_else(|| FactorioToolsError::new("An item should have matched the predicate."))
}

/// `First()`. The C# reuses the predicate message here.
pub fn first<T>(items: impl IntoIterator<Item = T>) -> FtResult<T> {
    items
        .into_iter()
        .next()
        .ok_or_else(|| FactorioToolsError::new("An item should have matched the predicate."))
}

/// `First(predicate)`.
pub fn first_where<T>(
    items: impl IntoIterator<Item = T>,
    predicate: impl FnMut(&T) -> bool,
) -> FtResult<T> {
    items
        .into_iter()
        .find(predicate)
        .ok_or_else(|| FactorioToolsError::new("An item should have matched the predicate."))
}

/// `Average(selector)` over ints: 0 for an empty source, otherwise a `double` sum divided
/// by the count, adding in order like the C#.
pub fn average(items: impl IntoIterator<Item = i32>) -> f64 {
    let mut sum = 0.0f64;
    let mut count = 0u32;
    for item in items {
        sum += f64::from(item);
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / f64::from(count)
    }
}

/// `Sum(selector)` over ints. C# `int` addition wraps on overflow outside `checked`.
pub fn sum(items: impl IntoIterator<Item = i32>) -> i32 {
    items.into_iter().fold(0i32, i32::wrapping_add)
}

/// `Distinct(context)`: the locations in first-seen order. The C# tracks what it has seen
/// in `context.GetLocationSet(count)`, a `LocationBitSet` the size of the grid, so this
/// takes the grid size.
pub fn distinct(locations: &[Location], grid_width: i32, grid_height: i32) -> Vec<Location> {
    let mut seen = LocationSet::new_bit(grid_width, grid_height);
    let mut output = Vec::with_capacity(locations.len());
    for &location in locations {
        if seen.add(location) {
            output.push(location);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_of_equal_keys_wins() {
        let items = [(1, 'a'), (3, 'b'), (3, 'c'), (0, 'd'), (0, 'e')];
        assert_eq!(max_by(items, |i| i.0), Some((3, 'b')));
        assert_eq!(min_by(items, |i| i.0), Some((0, 'd')));
        assert_eq!(max_by(Vec::<(i32, char)>::new(), |i| i.0), None);
    }

    #[test]
    fn float_keys_compare_like_double_compare_to() {
        let items = [1.0, f64::NAN, 2.0];
        assert_eq!(min_by(items, |x| *x).map(f64::is_nan), Some(true));
        assert_eq!(max(items), Some(2.0));
        assert_eq!(min([3, 1, 2]), Some(1));
    }

    #[test]
    fn single_and_first_messages() {
        assert_eq!(single([7]).unwrap(), 7);
        assert_eq!(
            single([1, 2]).unwrap_err().message,
            "Only one item should exist in the source."
        );
        assert_eq!(
            single(Vec::<i32>::new()).unwrap_err().message,
            "An item should exist in the source."
        );
        assert_eq!(single_where([1, 2, 3], |&x| x == 2).unwrap(), 2);
        assert_eq!(first_where([1, 2, 3], |&x| x > 1).unwrap(), 2);
        assert!(first(Vec::<i32>::new()).is_err());
    }

    #[test]
    fn numbers() {
        assert_eq!(average([1, 2]), 1.5);
        assert_eq!(average(Vec::new()), 0.0);
        assert_eq!(sum([i32::MAX, 1]), i32::MIN);
    }

    #[test]
    fn distinct_keeps_first_seen_order() {
        let l = |x, y| Location::new(x, y);
        assert_eq!(
            distinct(&[l(1, 1), l(0, 0), l(1, 1), l(2, 0)], 3, 3),
            vec![l(1, 1), l(0, 0), l(2, 0)]
        );
    }
}
