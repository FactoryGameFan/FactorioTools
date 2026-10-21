//! `ILocationDictionary<T>` as the default C# build implements it, `LocationHashDictionary<T>`
//! (`OilField/Containers/LocationHashDictionary.cs`): a `Dictionary<Location, T>`.
//!
//! The other build's `LocationIntDictionary<T>` is left out for the same reason as
//! `LocationIntSet` (see `location_set.rs`).

use dotnet_compat::NetDictionary;

use crate::Location;

/// `LocationHashDictionary<T>`, enumerating in `Dictionary<Location, T>` order.
#[derive(Debug)]
pub struct LocationDictionary<T> {
    dictionary: NetDictionary<Location, T>,
}

impl<T> Default for LocationDictionary<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LocationDictionary<T> {
    /// `new LocationHashDictionary<T>()`.
    pub fn new() -> Self {
        LocationDictionary {
            dictionary: NetDictionary::new(),
        }
    }

    /// `new LocationHashDictionary<T>(capacity)`.
    pub fn with_capacity(capacity: usize) -> Self {
        LocationDictionary {
            dictionary: NetDictionary::with_capacity(capacity),
        }
    }

    /// `Count`.
    pub fn len(&self) -> usize {
        self.dictionary.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dictionary.is_empty()
    }

    /// `Keys`, in .NET's order.
    pub fn keys(&self) -> impl Iterator<Item = Location> + '_ {
        self.dictionary.keys()
    }

    /// `Values`, in .NET's order.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.dictionary.values()
    }

    /// `EnumeratePairs`, in .NET's order.
    pub fn iter(&self) -> impl Iterator<Item = (Location, &T)> {
        self.dictionary.iter().map(|(k, v)| (*k, v))
    }

    /// The indexer's getter. Panics like `KeyNotFoundException` when the key is missing.
    pub fn value(&self, key: Location) -> &T {
        self.dictionary
            .get(&key)
            .unwrap_or_else(|| panic!("KeyNotFoundException: {key:?}"))
    }

    /// The indexer's getter, for changing the value in place.
    pub fn value_mut(&mut self, key: Location) -> &mut T {
        self.dictionary
            .get_mut(&key)
            .unwrap_or_else(|| panic!("KeyNotFoundException: {key:?}"))
    }

    /// The indexer's setter: adds the key, or replaces its value where it stands.
    pub fn set(&mut self, key: Location, value: T) {
        self.dictionary.set(key, value);
    }

    /// `Add`. Panics like `ArgumentException` when the key is already present.
    pub fn add(&mut self, key: Location, value: T) {
        self.dictionary.add(key, value);
    }

    /// `Clear`.
    pub fn clear(&mut self) {
        self.dictionary.clear();
    }

    /// `ContainsKey`.
    pub fn contains_key(&self, key: Location) -> bool {
        self.dictionary.contains_key(&key)
    }

    /// `Remove`.
    pub fn remove(&mut self, key: Location) -> bool {
        self.dictionary.remove(&key).is_some()
    }

    /// `Remove`, returning the removed value.
    pub fn take(&mut self, key: Location) -> Option<T> {
        self.dictionary.remove(&key)
    }

    /// `TryAdd`.
    pub fn try_add(&mut self, key: Location, value: T) -> bool {
        self.dictionary.try_add(key, value)
    }

    /// `TryGetValue`.
    pub fn get(&self, key: Location) -> Option<&T> {
        self.dictionary.get(&key)
    }

    /// `TryGetValue`, for changing the value in place.
    pub fn get_mut(&mut self, key: Location) -> Option<&mut T> {
        self.dictionary.get_mut(&key)
    }

    /// The underlying .NET-ordered dictionary.
    pub fn inner(&self) -> &NetDictionary<Location, T> {
        &self.dictionary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_dotnet_order() {
        let mut d = LocationDictionary::new();
        d.add(Location::new(1, 1), 'a');
        d.add(Location::new(2, 2), 'b');
        d.add(Location::new(3, 3), 'c');
        assert!(d.remove(Location::new(2, 2)));
        d.set(Location::new(4, 4), 'd');
        d.set(Location::new(1, 1), 'e');
        let pairs: Vec<_> = d.iter().map(|(k, v)| (k, *v)).collect();
        assert_eq!(
            pairs,
            vec![
                (Location::new(1, 1), 'e'),
                (Location::new(4, 4), 'd'),
                (Location::new(3, 3), 'c'),
            ]
        );
        assert!(!d.try_add(Location::new(4, 4), 'x'));
        assert_eq!(*d.value(Location::new(3, 3)), 'c');
    }
}
