//! The `ILocationSet` family, from `OilField/Containers`: `LocationHashSet.cs`,
//! `LocationBitSet.cs`, `SingleLocationSet.cs` and `EmptyLocationSet.cs`.
//!
//! The port copies the default C# build (`USE_HASHSETS`), where an enumerable set is a
//! `LocationHashSet` over `HashSet<Location>`. The other build's `LocationIntSet` is left out:
//! CI runs every snapshot under both and they agree, so one order is enough.
//!
//! The C# picks an implementation per call site through `Context.GetLocationSet`, and some
//! operations cast the other set to the same implementation and throw if it is not. The
//! port keeps those as one enum so callers keep the C#'s shape, and it panics where the C#
//! throws `InvalidCastException` or `NotSupportedException`, since those are planner bugs.

use dotnet_compat::NetHashSet;

use crate::Location;
use crate::containers::CountedBitArray;

/// An `ILocationSet`.
#[derive(Debug)]
pub enum LocationSet {
    /// `LocationHashSet`: enumerable, in `HashSet<Location>` order.
    Hash(LocationHashSet),
    /// `LocationBitSet`: one bit per grid cell, not enumerable.
    Bit(LocationBitSet),
    /// `SingleLocationSet`: at most one location.
    Single(SingleLocationSet),
    /// `EmptyLocationSet.Instance`: always empty, and `Add` throws.
    Empty,
}

impl LocationSet {
    /// `new LocationHashSet()`.
    pub fn new_hash() -> Self {
        LocationSet::Hash(LocationHashSet::new())
    }

    /// `new LocationHashSet(capacity)`.
    pub fn new_hash_with_capacity(capacity: usize) -> Self {
        LocationSet::Hash(LocationHashSet::with_capacity(capacity))
    }

    /// `new LocationBitSet(width, height)`.
    pub fn new_bit(width: i32, height: i32) -> Self {
        LocationSet::Bit(LocationBitSet::new(width, height))
    }

    /// `new SingleLocationSet(location)`.
    pub fn new_single(location: Location) -> Self {
        LocationSet::Single(SingleLocationSet::new(location))
    }

    /// `Count`.
    pub fn len(&self) -> usize {
        match self {
            LocationSet::Hash(s) => s.len(),
            LocationSet::Bit(s) => s.len(),
            LocationSet::Single(s) => s.len(),
            LocationSet::Empty => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether `EnumerateItems` works on this set. A bit set throws.
    pub fn is_enumerable(&self) -> bool {
        !matches!(self, LocationSet::Bit(_))
    }

    /// `Add`.
    pub fn add(&mut self, location: Location) -> bool {
        match self {
            LocationSet::Hash(s) => s.add(location),
            LocationSet::Bit(s) => s.add(location),
            LocationSet::Single(s) => s.add(location),
            LocationSet::Empty => panic!("NotSupportedException: EmptyLocationSet.Add"),
        }
    }

    /// `Clear`.
    pub fn clear(&mut self) {
        match self {
            LocationSet::Hash(s) => s.set.clear(),
            LocationSet::Bit(s) => s.bits.set_all(false),
            LocationSet::Single(s) => s.clear(),
            LocationSet::Empty => {}
        }
    }

    /// `Contains`.
    pub fn contains(&self, location: Location) -> bool {
        match self {
            LocationSet::Hash(s) => s.contains(location),
            LocationSet::Bit(s) => s.contains(location),
            LocationSet::Single(s) => s.contains(location),
            LocationSet::Empty => false,
        }
    }

    /// `Remove`.
    pub fn remove(&mut self, location: Location) -> bool {
        match self {
            LocationSet::Hash(s) => s.set.remove(&location),
            LocationSet::Bit(s) => s.remove(location),
            LocationSet::Single(s) => s.remove(location),
            LocationSet::Empty => false,
        }
    }

    /// `EnumerateItems`, in the C#'s order. Panics for a bit set, like the C#.
    pub fn iter(&self) -> Box<dyn Iterator<Item = Location> + '_> {
        match self {
            LocationSet::Hash(s) => Box::new(s.set.iter()),
            LocationSet::Bit(_) => {
                panic!("NotSupportedException: LocationBitSet.EnumerateItems")
            }
            LocationSet::Single(s) => Box::new(s.item().into_iter()),
            LocationSet::Empty => Box::new(std::iter::empty()),
        }
    }

    /// `EnumerateItems`, collected.
    pub fn to_vec(&self) -> Vec<Location> {
        self.iter().collect()
    }

    /// `CopyTo(Span<Location>)`.
    pub fn copy_to(&self, array: &mut [Location]) {
        match self {
            LocationSet::Hash(s) => {
                for (slot, item) in array.iter_mut().zip(s.set.iter()) {
                    *slot = item;
                }
            }
            LocationSet::Bit(_) => panic!("NotSupportedException: LocationBitSet.CopyTo"),
            LocationSet::Single(s) => {
                if let Some(item) = s.item() {
                    if array.is_empty() {
                        panic!("ArgumentException: The array is not big enough.");
                    }
                    array[0] = item;
                }
            }
            LocationSet::Empty => {}
        }
    }

    /// `ExceptWith(ILocationSet)`.
    pub fn except_with(&mut self, other: &LocationSet) {
        match self {
            LocationSet::Hash(s) => {
                if other.is_empty() {
                    return;
                }
                for item in other.iter() {
                    s.set.remove(&item);
                }
            }
            LocationSet::Bit(s) => {
                let other = s.validate_same_dimensions(other);
                let mut other_bits = other.bits.clone();
                other_bits.not();
                s.bits.and(&other_bits);
            }
            LocationSet::Single(_) => panic!("NotSupportedException: SingleLocationSet.ExceptWith"),
            LocationSet::Empty => {}
        }
    }

    /// `Overlaps(IReadOnlyCollection<Location>)`.
    pub fn overlaps(&self, other: impl IntoIterator<Item = Location>) -> bool {
        match self {
            LocationSet::Hash(s) => s.set.overlaps(other),
            LocationSet::Bit(s) => other.into_iter().any(|item| s.contains(item)),
            LocationSet::Single(_) => panic!("NotSupportedException: SingleLocationSet.Overlaps"),
            LocationSet::Empty => false,
        }
    }

    /// `SetEquals(ILocationSet)`.
    pub fn set_equals(&self, other: &LocationSet) -> bool {
        match self {
            LocationSet::Hash(s) => match other {
                LocationSet::Hash(o) => s.set.set_equals(o.set.iter()),
                _ => panic!(
                    "InvalidCastException: LocationHashSet.SetEquals needs a LocationHashSet"
                ),
            },
            LocationSet::Bit(s) => {
                s.validate_same_dimensions(other);
                // The C# calls `_set.Equals(otherSet)` with the other LocationBitSet, not its
                // bit array. That binds to `Equals(object)`, whose `as CustomCountedBitArray`
                // gives null, so the answer is always false. Copied as it is.
                false
            }
            // The C# checks the stored coordinates even when the set is empty: (-1, -1) after
            // `Remove`, or the old item after `Clear`, which only drops the flag.
            LocationSet::Single(s) => other.len() == 1 && other.contains(Location::new(s.x, s.y)),
            LocationSet::Empty => other.is_empty(),
        }
    }

    /// `UnionWith(IReadOnlyCollection<Location>)`.
    pub fn union_with_items(&mut self, other: impl IntoIterator<Item = Location>) {
        match self {
            LocationSet::Hash(s) => s.set.union_with(other),
            LocationSet::Bit(s) => {
                for item in other {
                    s.add(item);
                }
            }
            LocationSet::Single(_) => panic!("NotSupportedException: SingleLocationSet.UnionWith"),
            LocationSet::Empty => panic!("NotSupportedException: EmptyLocationSet.UnionWith"),
        }
    }

    /// `UnionWith(ILocationSet)`.
    pub fn union_with(&mut self, other: &LocationSet) {
        match self {
            LocationSet::Hash(s) => {
                // The C# adds a single item through `EnumerateItems`, and otherwise casts the
                // other set to `LocationHashSet` for `HashSet.UnionWith`. Both add in the
                // other set's order.
                match other.len() {
                    0 => {}
                    1 => {
                        for item in other.iter() {
                            s.set.add(item);
                        }
                    }
                    _ => match other {
                        LocationSet::Hash(o) => s.set.union_with(o.set.iter()),
                        _ => panic!(
                            "InvalidCastException: LocationHashSet.UnionWith needs a LocationHashSet"
                        ),
                    },
                }
            }
            LocationSet::Bit(s) => {
                let other = s.validate_same_dimensions(other);
                s.bits.or(&other.bits);
            }
            LocationSet::Single(_) => panic!("NotSupportedException: SingleLocationSet.UnionWith"),
            LocationSet::Empty => panic!("NotSupportedException: EmptyLocationSet.UnionWith"),
        }
    }

    /// `Context.GetLocationSet(ILocationSet other)`: a copy with the same implementation.
    /// A hash set goes through `HashSet<T>`'s copy constructor, which can keep freed slots.
    pub fn copy(&self) -> LocationSet {
        match self {
            LocationSet::Hash(s) => LocationSet::Hash(LocationHashSet {
                set: NetHashSet::copy_from(&s.set),
            }),
            LocationSet::Single(s) => LocationSet::Single(s.clone()),
            LocationSet::Bit(s) => LocationSet::Bit(s.clone()),
            LocationSet::Empty => {
                panic!("InvalidCastException: EmptyLocationSet is not a LocationBitSet")
            }
        }
    }
}

/// `LocationHashSet`.
#[derive(Debug)]
pub struct LocationHashSet {
    pub(crate) set: NetHashSet<Location>,
}

impl Default for LocationHashSet {
    fn default() -> Self {
        Self::new()
    }
}

impl LocationHashSet {
    pub fn new() -> Self {
        LocationHashSet {
            set: NetHashSet::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        LocationHashSet {
            set: NetHashSet::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    pub fn add(&mut self, location: Location) -> bool {
        self.set.add(location)
    }

    pub fn contains(&self, location: Location) -> bool {
        self.set.contains(&location)
    }

    /// The underlying .NET-ordered set.
    pub fn inner(&self) -> &NetHashSet<Location> {
        &self.set
    }
}

/// `LocationBitSet`.
#[derive(Clone, Debug)]
pub struct LocationBitSet {
    width: i32,
    bits: CountedBitArray,
}

impl LocationBitSet {
    pub fn new(width: i32, height: i32) -> Self {
        LocationBitSet {
            width,
            bits: CountedBitArray::new(width * height),
        }
    }

    pub fn len(&self) -> usize {
        self.bits.true_count() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `GetIndex`: `Y * width + X`, with no bounds check. A location one cell past the
    /// right edge is the first cell of the next row, the same as in the C#.
    fn index(&self, location: Location) -> i32 {
        location.y * self.width + location.x
    }

    pub fn add(&mut self, location: Location) -> bool {
        let index = self.index(location);
        !self.bits.set(index, true)
    }

    pub fn contains(&self, location: Location) -> bool {
        self.bits.get(self.index(location))
    }

    pub fn remove(&mut self, location: Location) -> bool {
        let index = self.index(location);
        self.bits.set(index, false)
    }

    /// `ValidateSameDimensions`: casts the other set and compares widths only.
    fn validate_same_dimensions<'a>(&self, other: &'a LocationSet) -> &'a LocationBitSet {
        match other {
            LocationSet::Bit(o) => {
                if o.width != self.width {
                    panic!("ArgumentException: The location set dimensions must be the same.");
                }
                o
            }
            _ => panic!("InvalidCastException: expected a LocationBitSet"),
        }
    }
}

/// `SingleLocationSet`.
#[derive(Clone, Debug)]
pub struct SingleLocationSet {
    x: i32,
    y: i32,
    has_item: bool,
}

impl Default for SingleLocationSet {
    /// `new SingleLocationSet()`.
    fn default() -> Self {
        SingleLocationSet {
            x: -1,
            y: -1,
            has_item: false,
        }
    }
}

impl SingleLocationSet {
    pub fn new(location: Location) -> Self {
        SingleLocationSet {
            x: location.x,
            y: location.y,
            has_item: true,
        }
    }

    pub fn len(&self) -> usize {
        usize::from(self.has_item)
    }

    pub fn is_empty(&self) -> bool {
        !self.has_item
    }

    fn item(&self) -> Option<Location> {
        self.has_item.then(|| Location::new(self.x, self.y))
    }

    pub fn add(&mut self, location: Location) -> bool {
        if !self.has_item {
            self.x = location.x;
            self.y = location.y;
            self.has_item = true;
            true
        } else if self.x == location.x && self.y == location.y {
            false
        } else {
            panic!("NotSupportedException: SingleLocationSet already has an item");
        }
    }

    /// `Clear` only drops the flag; the old coordinates stay, as in the C#.
    pub fn clear(&mut self) {
        self.has_item = false;
    }

    pub fn contains(&self, location: Location) -> bool {
        self.has_item && location.x == self.x && location.y == self.y
    }

    pub fn remove(&mut self, location: Location) -> bool {
        if self.has_item && self.x == location.x && self.y == location.y {
            self.x = -1;
            self.y = -1;
            self.has_item = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(x: i32, y: i32) -> Location {
        Location::new(x, y)
    }

    #[test]
    fn hash_set_keeps_dotnet_order() {
        let mut set = LocationSet::new_hash();
        for x in 0..4 {
            set.add(loc(x, 0));
        }
        set.remove(loc(1, 0));
        set.add(loc(9, 9));
        assert_eq!(
            set.to_vec(),
            vec![loc(0, 0), loc(9, 9), loc(2, 0), loc(3, 0)]
        );
    }

    #[test]
    fn union_and_except() {
        let mut a = LocationSet::new_hash();
        a.union_with_items([loc(1, 1), loc(2, 2)]);
        let mut b = LocationSet::new_hash();
        b.union_with_items([loc(2, 2), loc(3, 3)]);
        a.union_with(&b);
        assert_eq!(a.to_vec(), vec![loc(1, 1), loc(2, 2), loc(3, 3)]);
        a.except_with(&b);
        assert_eq!(a.to_vec(), vec![loc(1, 1)]);
        assert!(a.set_equals(&{
            let mut c = LocationSet::new_hash();
            c.add(loc(1, 1));
            c
        }));
    }

    #[test]
    fn bit_set_operations() {
        let mut a = LocationSet::new_bit(5, 5);
        assert!(a.add(loc(1, 2)));
        assert!(!a.add(loc(1, 2)));
        let mut b = LocationSet::new_bit(5, 5);
        b.add(loc(4, 4));
        a.union_with(&b);
        assert_eq!(a.len(), 2);
        a.except_with(&b);
        assert_eq!(a.len(), 1);
        assert!(a.contains(loc(1, 2)));
        assert!(a.overlaps([loc(0, 0), loc(1, 2)]));
        // Always false, as in the C#.
        assert!(!a.set_equals(&a.copy()));
    }

    #[test]
    fn bit_set_wraps_past_the_right_edge() {
        let mut a = LocationSet::new_bit(5, 5);
        a.add(loc(0, 1));
        assert!(a.contains(loc(5, 0)));
    }

    #[test]
    #[should_panic(expected = "NotSupportedException")]
    fn bit_set_does_not_enumerate() {
        let a = LocationSet::new_bit(2, 2);
        let _ = a.to_vec();
    }

    #[test]
    fn single_and_empty() {
        let mut s = LocationSet::new_single(loc(3, 4));
        assert_eq!(s.to_vec(), vec![loc(3, 4)]);
        assert!(!s.add(loc(3, 4)));
        assert!(s.remove(loc(3, 4)));
        assert!(s.is_empty());
        let e = LocationSet::Empty;
        assert!(e.set_equals(&LocationSet::new_hash()));
        assert!(!e.contains(loc(0, 0)));
    }
}
