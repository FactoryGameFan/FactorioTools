//! `HashSet<T>` and `Dictionary<TKey, TValue>` from System.Collections.Generic (.NET 10).
//!
//! Both store items in an `_entries` array and enumerate it in index order, skipping freed
//! entries. A removed entry goes on a free list, and the next add reuses the most recently
//! freed entry first. Resizing copies entries in order. So the enumeration order depends
//! only on the sequence of adds and removes, and on the capacity rules in a few places,
//! never on hash codes. That is what this module copies.
//!
//! Lookups go through a std `HashMap` from key to entry index, since bucket chains in .NET
//! never affect the order anything is observed in.

use std::collections::HashMap;
use std::hash::Hash;

use crate::fast_hash::FastBuildHasher;

use crate::hash_helpers::{expand_prime, get_prime};

/// .NET marks the end of the free list with -1, and stores a freed entry's link as
/// `StartOfFreeList - next`. Here the link is stored directly.
const END_OF_FREE_LIST: isize = -1;

#[derive(Clone, Debug)]
enum Slot<K, V> {
    Used(K, V),
    /// A freed entry, holding the index of the next free entry (or `END_OF_FREE_LIST`).
    Free(isize),
}

/// The entry array, free list and capacity shared by `HashSet` and `Dictionary`.
#[derive(Clone, Debug)]
struct Table<K, V> {
    /// `_entries[0.._count]`. Its length is .NET's `_count`, the high-water mark.
    slots: Vec<Slot<K, V>>,
    /// Key to entry index, standing in for `_buckets`. Only looked up, so its hasher is
    /// free to be fast (see [`crate::fast_hash`]).
    index: HashMap<K, usize, FastBuildHasher>,
    /// `_entries.Length` (equal to `_buckets.Length`), or 0 while `_buckets` is null.
    capacity: usize,
    /// `_freeList`.
    free_list: isize,
    /// `_freeCount`.
    free_count: usize,
}

impl<K: Copy + Eq + Hash, V> Table<K, V> {
    fn new() -> Self {
        Table {
            slots: Vec::new(),
            index: HashMap::default(),
            capacity: 0,
            free_list: END_OF_FREE_LIST,
            free_count: 0,
        }
    }

    /// The `(int capacity)` constructors: allocate only when the capacity is positive.
    fn with_capacity(capacity: usize) -> Self {
        let mut table = Self::new();
        if capacity > 0 {
            table.initialize(capacity);
        }
        table
    }

    /// `Initialize(int capacity)`: allocate fresh, empty arrays of a prime size.
    fn initialize(&mut self, capacity: usize) -> usize {
        let size = get_prime(capacity);
        self.slots = Vec::with_capacity(size);
        self.index = HashMap::with_capacity_and_hasher(size, FastBuildHasher::default());
        self.capacity = size;
        self.free_list = END_OF_FREE_LIST;
        self.free_count = 0;
        size
    }

    /// `Count`.
    fn len(&self) -> usize {
        self.slots.len() - self.free_count
    }

    fn contains(&self, key: &K) -> bool {
        self.index.contains_key(key)
    }

    fn get(&self, key: &K) -> Option<&V> {
        match self.index.get(key).map(|&i| &self.slots[i]) {
            Some(Slot::Used(_, v)) => Some(v),
            _ => None,
        }
    }

    fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let i = *self.index.get(key)?;
        match &mut self.slots[i] {
            Slot::Used(_, v) => Some(v),
            Slot::Free(_) => None,
        }
    }

    /// The new-entry half of `AddIfNotPresent` / `TryInsert`. The caller has checked that
    /// the key is not present.
    fn insert_new(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            self.initialize(0);
        }

        let index = if self.free_count > 0 {
            // Reuse the most recently freed entry.
            let index = self.free_list as usize;
            self.free_count -= 1;
            self.free_list = match self.slots[index] {
                Slot::Free(next) => next,
                Slot::Used(..) => unreachable!("free list points at a used entry"),
            };
            self.slots[index] = Slot::Used(key, value);
            index
        } else {
            let count = self.slots.len();
            if count == self.capacity {
                // `Resize()`: entries keep their order, only the capacity grows.
                self.capacity = expand_prime(count);
            }
            self.slots.push(Slot::Used(key, value));
            count
        };

        self.index.insert(key, index);
    }

    /// `Remove`: free the entry and push it on the free list.
    fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.index.remove(key)?;
        let old = std::mem::replace(&mut self.slots[index], Slot::Free(self.free_list));
        self.free_list = index as isize;
        self.free_count += 1;
        match old {
            Slot::Used(_, v) => Some(v),
            Slot::Free(_) => unreachable!("index pointed at a free entry"),
        }
    }

    /// `Clear`: empty the arrays but keep their capacity.
    fn clear(&mut self) {
        if !self.slots.is_empty() {
            self.slots.clear();
            self.index.clear();
            self.free_list = END_OF_FREE_LIST;
            self.free_count = 0;
        }
    }

    /// `EnsureCapacity(int capacity)`.
    fn ensure_capacity(&mut self, capacity: usize) -> usize {
        if self.capacity >= capacity {
            return self.capacity;
        }
        if self.capacity == 0 {
            return self.initialize(capacity);
        }
        // `Resize(newSize)`: entries keep their order.
        let new_size = get_prime(capacity);
        self.capacity = new_size;
        new_size
    }

    /// The used entries, in order, with no gaps: what `CopyEntries` and `TrimExcess` keep.
    fn compact(&mut self) -> Vec<(K, V)> {
        std::mem::take(&mut self.slots)
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Used(k, v) => Some((k, v)),
                Slot::Free(_) => None,
            })
            .collect()
    }

    /// Fill a freshly initialized table with entries in order: `CopyEntries`.
    fn copy_entries(&mut self, entries: Vec<(K, V)>) {
        for (i, (k, v)) in entries.into_iter().enumerate() {
            self.index.insert(k, i);
            self.slots.push(Slot::Used(k, v));
        }
        self.free_list = END_OF_FREE_LIST;
        self.free_count = 0;
    }

    /// `TrimExcess(int capacity)`: shrink to `GetPrime(capacity)` if that is smaller, which
    /// also drops the freed entries.
    fn trim_excess_to(&mut self, capacity: usize) {
        assert!(capacity >= self.len(), "capacity is less than Count");
        let new_size = get_prime(capacity);
        if new_size >= self.capacity {
            return;
        }
        let entries = self.compact();
        self.initialize(new_size);
        self.copy_entries(entries);
    }

    fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Used(k, v) => Some((k, v)),
            Slot::Free(_) => None,
        })
    }
}

/// A copy of .NET's `HashSet<T>` that enumerates in the same order.
#[derive(Debug)]
pub struct NetHashSet<T> {
    table: Table<T, ()>,
}

impl<T: Copy + Eq + Hash> Default for NetHashSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Eq + Hash> NetHashSet<T> {
    /// `new HashSet<T>()`.
    pub fn new() -> Self {
        NetHashSet {
            table: Table::new(),
        }
    }

    /// `new HashSet<T>(int capacity)`.
    pub fn with_capacity(capacity: usize) -> Self {
        NetHashSet {
            table: Table::with_capacity(capacity),
        }
    }

    /// `new HashSet<T>(HashSet<T> other)`, where both use the default comparer.
    ///
    /// This is `ConstructFrom`. When `ExpandPrime(source.Count + 1)` is at least the
    /// source's capacity, it clones the source's arrays as they are, freed entries and free
    /// list included, so later adds fill the same holes the source would. Otherwise it
    /// allocates `GetPrime(Count)` and re-adds the items in order, which leaves no holes.
    pub fn copy_from(source: &NetHashSet<T>) -> Self {
        if source.is_empty() {
            return Self::new();
        }
        let capacity = source.table.capacity;
        let threshold = expand_prime(source.len() + 1);
        if threshold >= capacity {
            return NetHashSet {
                table: source.table.clone(),
            };
        }
        let mut set = Self::new();
        set.table.initialize(source.len());
        for item in source.iter() {
            set.add(item);
        }
        set
    }

    /// `new HashSet<T>(IEnumerable<T> collection)` for a collection that is an
    /// `ICollection<T>` but not a `HashSet<T>` (a `List<T>` or an array), so `count` is
    /// known up front.
    pub fn from_collection(items: &[T]) -> Self {
        let mut set = Self::new();
        if !items.is_empty() {
            set.table.initialize(items.len());
        }
        set.union_with(items.iter().copied());
        // `ShrinkThreshold` is 3: trim when the arrays are over three times the count.
        let count = set.table.slots.len();
        if count > 0 && set.table.capacity / count > 3 {
            set.trim_excess();
        }
        set
    }

    /// `new HashSet<T>(IEnumerable<T> collection)` for a lazy sequence with no known count.
    pub fn from_iter_lazy(items: impl IntoIterator<Item = T>) -> Self {
        let mut set = Self::new();
        set.union_with(items);
        let count = set.table.slots.len();
        if count > 0 && set.table.capacity / count > 3 {
            set.trim_excess();
        }
        set
    }

    /// `Count`.
    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.len() == 0
    }

    /// The size of the entry array (`_entries.Length`), 0 before the first allocation.
    pub fn capacity(&self) -> usize {
        self.table.capacity
    }

    /// `Add`. Returns false if the item was already present.
    pub fn add(&mut self, item: T) -> bool {
        if self.table.contains(&item) {
            return false;
        }
        self.table.insert_new(item, ());
        true
    }

    /// `Remove`. Returns false if the item was not present.
    pub fn remove(&mut self, item: &T) -> bool {
        self.table.remove(item).is_some()
    }

    /// `Contains`.
    pub fn contains(&self, item: &T) -> bool {
        self.table.contains(item)
    }

    /// `Clear`. Keeps the capacity, and forgets the free list.
    pub fn clear(&mut self) {
        self.table.clear();
    }

    /// `EnsureCapacity`. Returns the capacity afterwards.
    pub fn ensure_capacity(&mut self, capacity: usize) -> usize {
        self.table.ensure_capacity(capacity)
    }

    /// `TrimExcess()`.
    pub fn trim_excess(&mut self) {
        let count = self.len();
        self.table.trim_excess_to(count);
    }

    /// `TrimExcess(int capacity)`.
    pub fn trim_excess_to(&mut self, capacity: usize) {
        self.table.trim_excess_to(capacity);
    }

    /// Enumerates in .NET's order.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.table.iter().map(|(k, _)| *k)
    }

    /// `First()` / the first item enumerated.
    pub fn first(&self) -> Option<T> {
        self.iter().next()
    }

    /// `CopyTo`: the items in enumeration order.
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().collect()
    }

    /// `UnionWith`: add each item in the order given.
    pub fn union_with(&mut self, other: impl IntoIterator<Item = T>) {
        for item in other {
            self.add(item);
        }
    }

    /// `ExceptWith`: remove each item in the order given.
    pub fn except_with(&mut self, other: impl IntoIterator<Item = T>) {
        if self.is_empty() {
            return;
        }
        for item in other {
            self.remove(&item);
        }
    }

    /// `IntersectWith` with another `HashSet<T>` using the same comparer
    /// (`IntersectWithHashSetWithSameComparer`): walk this set's entries in order and remove
    /// each one the other set lacks. An empty `other` clears this set instead.
    pub fn intersect_with_set(&mut self, other: &NetHashSet<T>) {
        if self.is_empty() {
            return;
        }
        if other.is_empty() {
            self.clear();
            return;
        }
        let count = self.table.slots.len();
        for i in 0..count {
            if let Slot::Used(item, ()) = self.table.slots[i]
                && !other.contains(&item)
            {
                self.remove(&item);
            }
        }
    }

    /// `IntersectWith` with an `ICollection<T>` that is not a `HashSet<T>` (a `List<T>` or an
    /// array). An empty collection clears this set; otherwise this is
    /// `IntersectWithEnumerable`: mark the entries found in `other`, then remove the
    /// unmarked ones in entry order.
    pub fn intersect_with_collection(&mut self, other: &[T]) {
        if self.is_empty() {
            return;
        }
        if other.is_empty() {
            self.clear();
            return;
        }
        self.intersect_with_enumerable(other.iter().copied());
    }

    /// `IntersectWith` with a lazy sequence (`IntersectWithEnumerable`). Unlike a
    /// collection, an empty sequence removes entries one by one rather than clearing.
    pub fn intersect_with_iter(&mut self, other: impl IntoIterator<Item = T>) {
        if self.is_empty() {
            return;
        }
        self.intersect_with_enumerable(other);
    }

    fn intersect_with_enumerable(&mut self, other: impl IntoIterator<Item = T>) {
        let original_count = self.table.slots.len();
        let mut marked = vec![false; original_count];
        for item in other {
            if let Some(&i) = self.table.index.get(&item) {
                marked[i] = true;
            }
        }
        for (i, is_marked) in marked.into_iter().enumerate() {
            if let Slot::Used(item, ()) = self.table.slots[i]
                && !is_marked
            {
                self.remove(&item);
            }
        }
    }

    /// `Overlaps`.
    pub fn overlaps(&self, other: impl IntoIterator<Item = T>) -> bool {
        if self.is_empty() {
            return false;
        }
        other.into_iter().any(|item| self.contains(&item))
    }

    /// `SetEquals`: true when `other`, taken as a set, has exactly this set's items.
    pub fn set_equals(&self, other: impl IntoIterator<Item = T>) -> bool {
        let mut seen = std::collections::HashSet::new();
        for item in other {
            if !self.contains(&item) {
                return false;
            }
            seen.insert(item);
        }
        seen.len() == self.len()
    }

    /// `IsSubsetOf`.
    pub fn is_subset_of(&self, other: &NetHashSet<T>) -> bool {
        self.len() <= other.len() && self.iter().all(|item| other.contains(&item))
    }
}

/// A copy of .NET's `Dictionary<TKey, TValue>` that enumerates in the same order.
#[derive(Debug)]
pub struct NetDictionary<K, V> {
    table: Table<K, V>,
}

impl<K: Copy + Eq + Hash, V> Default for NetDictionary<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Copy + Eq + Hash, V> NetDictionary<K, V> {
    /// `new Dictionary<TKey, TValue>()`.
    pub fn new() -> Self {
        NetDictionary {
            table: Table::new(),
        }
    }

    /// `new Dictionary<TKey, TValue>(int capacity)`.
    pub fn with_capacity(capacity: usize) -> Self {
        NetDictionary {
            table: Table::with_capacity(capacity),
        }
    }

    /// `new Dictionary<TKey, TValue>(Dictionary<TKey, TValue> source)`, where both use the
    /// default comparer. It allocates `GetPrime(source.Count)` and copies the used entries
    /// in order (`AddRange` then `CopyEntries`), so unlike `HashSet` the copy never keeps
    /// freed entries.
    pub fn copy_from(source: &NetDictionary<K, V>) -> Self
    where
        V: Clone,
    {
        let mut dict = Self::with_capacity(source.len());
        if source.is_empty() {
            return dict;
        }
        let entries = source.iter().map(|(k, v)| (*k, v.clone())).collect();
        dict.table.copy_entries(entries);
        dict
    }

    /// `Count`.
    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.len() == 0
    }

    /// The size of the entry array (`_entries.Length`), 0 before the first allocation.
    pub fn capacity(&self) -> usize {
        self.table.capacity
    }

    /// The indexer setter, `dict[key] = value`: overwrite in place, or add at the end.
    pub fn set(&mut self, key: K, value: V) {
        if let Some(slot) = self.table.get_mut(&key) {
            *slot = value;
        } else {
            self.table.insert_new(key, value);
        }
    }

    /// `Add`. Panics if the key is already present, as .NET throws.
    pub fn add(&mut self, key: K, value: V) {
        assert!(
            !self.table.contains(&key),
            "An item with the same key has already been added."
        );
        self.table.insert_new(key, value);
    }

    /// `TryAdd`. Returns false, and changes nothing, if the key is already present.
    pub fn try_add(&mut self, key: K, value: V) -> bool {
        if self.table.contains(&key) {
            return false;
        }
        self.table.insert_new(key, value);
        true
    }

    /// `TryGetValue`.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.table.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.table.get_mut(key)
    }

    /// `ContainsKey`.
    pub fn contains_key(&self, key: &K) -> bool {
        self.table.contains(key)
    }

    /// `Remove(key, out value)`.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.table.remove(key)
    }

    /// `Clear`. Keeps the capacity, and forgets the free list.
    pub fn clear(&mut self) {
        self.table.clear();
    }

    /// `EnsureCapacity`. Returns the capacity afterwards.
    pub fn ensure_capacity(&mut self, capacity: usize) -> usize {
        self.table.ensure_capacity(capacity)
    }

    /// `TrimExcess()`.
    pub fn trim_excess(&mut self) {
        let count = self.len();
        self.table.trim_excess_to(count);
    }

    /// `TrimExcess(int capacity)`.
    pub fn trim_excess_to(&mut self, capacity: usize) {
        self.table.trim_excess_to(capacity);
    }

    /// Enumerates key-value pairs in .NET's order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.table.iter()
    }

    /// `Keys`, in .NET's order.
    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.table.iter().map(|(k, _)| *k)
    }

    /// `Values`, in .NET's order.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.table.iter().map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_entries_are_reused_last_in_first_out() {
        let mut set = NetHashSet::new();
        for i in 0..5 {
            set.add(i);
        }
        set.remove(&1);
        set.remove(&3);
        set.add(10); // takes 3's entry
        set.add(11); // takes 1's entry
        set.add(12); // appended
        assert_eq!(set.to_vec(), vec![0, 11, 2, 10, 4, 12]);
    }

    #[test]
    fn clear_forgets_the_free_list_but_keeps_capacity() {
        let mut set = NetHashSet::with_capacity(10);
        set.union_with([5, 6, 7]);
        set.remove(&5);
        set.clear();
        set.add(9);
        set.add(8);
        assert_eq!(set.to_vec(), vec![9, 8]);
        assert_eq!(set.capacity(), 11);
    }

    #[test]
    fn hash_set_copy_keeps_holes_when_capacity_is_small() {
        // 3 items in capacity 7: ExpandPrime(3) = 7 >= 7, so the arrays are cloned.
        let mut set = NetHashSet::new();
        set.union_with([1, 2, 3, 4]);
        set.remove(&2);
        let mut copy = NetHashSet::copy_from(&set);
        copy.add(9);
        assert_eq!(copy.to_vec(), vec![1, 9, 3, 4]);
    }

    #[test]
    fn dictionary_copy_compacts() {
        let mut dict = NetDictionary::new();
        for i in 0..4 {
            dict.set(i, i * 10);
        }
        dict.remove(&1);
        let mut copy = NetDictionary::copy_from(&dict);
        copy.set(9, 90);
        assert_eq!(copy.keys().collect::<Vec<_>>(), vec![0, 2, 3, 9]);
    }

    #[test]
    fn overwrite_keeps_position() {
        let mut dict = NetDictionary::new();
        dict.set(1, 'a');
        dict.set(2, 'b');
        dict.set(1, 'c');
        assert_eq!(
            dict.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
            vec![(1, 'c'), (2, 'b')]
        );
    }
}
