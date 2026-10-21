//! `SortedBatches<TInfo>`, from `OilField/Containers/SortedBatches.cs`.
//!
//! In the C#, the priority queue and the `Lookup` dictionary hold references to the same
//! `ILocationDictionary` objects, so a candidate moved through `Lookup` disappears from the
//! batch the queue will hand out later. Here the batches live in one `Vec`, and the queue
//! and the lookup hold indexes into it, which keeps that sharing.
//!
//! One C# detail carries over on purpose: `MoveCandidate` puts a candidate whose key is new
//! into a fresh batch that only `Lookup` knows about. That batch is never enqueued.

use dotnet_compat::{NetDictionary, NetPriorityQueue};

use crate::Location;
use crate::containers::LocationDictionary;

pub struct SortedBatches<TInfo> {
    batches: Vec<LocationDictionary<TInfo>>,
    /// `Queue`: batch indexes, by key (or by `-key` when descending).
    queue: NetPriorityQueue<usize, i32>,
    /// `Lookup`: key to batch index.
    lookup: NetDictionary<i32, usize>,
}

impl<TInfo> SortedBatches<TInfo> {
    /// `new SortedBatches(pairs, ascending)`. Batches are enqueued in the order given.
    pub fn new(
        pairs: impl IntoIterator<Item = (i32, LocationDictionary<TInfo>)>,
        ascending: bool,
    ) -> Self {
        let mut sorted = SortedBatches {
            batches: Vec::new(),
            queue: NetPriorityQueue::new(),
            lookup: NetDictionary::new(),
        };
        for (key, batch) in pairs {
            let index = sorted.batches.len();
            sorted.batches.push(batch);
            sorted
                .queue
                .enqueue(index, if ascending { key } else { -key });
            sorted.lookup.add(key, index);
        }
        sorted
    }

    /// `Queue.Count`.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// `Queue.Peek()`.
    pub fn peek(&self) -> Option<&LocationDictionary<TInfo>> {
        self.queue.peek().map(|&index| &self.batches[index])
    }

    /// `Queue.Peek()`, for changing the batch in place.
    pub fn peek_mut(&mut self) -> Option<&mut LocationDictionary<TInfo>> {
        let index = *self.queue.peek()?;
        Some(&mut self.batches[index])
    }

    /// Which batch `Queue.Peek()` returns, as an index for [`Self::batch_mut`]. The C# hands
    /// the batch object itself to code that then reaches it through `Lookup` too
    /// (`AddElectricPoles` passes it as `scopedCandidateToInfo`); the index keeps that one
    /// dictionary reachable both ways.
    pub fn peek_index(&self) -> Option<usize> {
        self.queue.peek().copied()
    }

    /// The batch at an index from [`Self::peek_index`].
    pub fn batch(&self, index: usize) -> &LocationDictionary<TInfo> {
        &self.batches[index]
    }

    /// The batch at an index from [`Self::peek_index`], for changing it in place.
    pub fn batch_mut(&mut self, index: usize) -> &mut LocationDictionary<TInfo> {
        &mut self.batches[index]
    }

    /// `Queue.Dequeue()`. The batch stays reachable through `Lookup`, as in the C#.
    pub fn dequeue(&mut self) -> Option<&LocationDictionary<TInfo>> {
        let index = self.queue.dequeue()?;
        Some(&self.batches[index])
    }

    /// `Lookup.TryGetValue(key)`.
    pub fn lookup(&self, key: i32) -> Option<&LocationDictionary<TInfo>> {
        self.lookup.get(&key).map(|&index| &self.batches[index])
    }

    /// `Lookup.TryGetValue(key)`, for changing the batch in place.
    pub fn lookup_mut(&mut self, key: i32) -> Option<&mut LocationDictionary<TInfo>> {
        let index = *self.lookup.get(&key)?;
        Some(&mut self.batches[index])
    }

    /// `Lookup` keys, in .NET's order.
    pub fn lookup_keys(&self) -> impl Iterator<Item = i32> + '_ {
        self.lookup.keys()
    }

    /// `RemoveCandidate(location, oldKey)`.
    pub fn remove_candidate(&mut self, location: Location, old_key: i32) {
        self.batch_for(old_key).remove(location);
    }

    /// `MoveCandidate(context, location, info, oldKey, newKey)`. The C# makes the new
    /// batch with `context.GetLocationDictionary<TInfo>()`, which in the default build is a
    /// plain `LocationHashDictionary`, so no context is needed here.
    pub fn move_candidate(&mut self, location: Location, info: TInfo, old_key: i32, new_key: i32) {
        self.batch_for(old_key).remove(location);
        if let Some(&index) = self.lookup.get(&new_key) {
            self.batches[index].add(location, info);
        } else {
            let mut batch = LocationDictionary::new();
            batch.add(location, info);
            let index = self.batches.len();
            self.batches.push(batch);
            self.lookup.add(new_key, index);
        }
    }

    fn batch_for(&mut self, key: i32) -> &mut LocationDictionary<TInfo> {
        let index = *self
            .lookup
            .get(&key)
            .unwrap_or_else(|| panic!("KeyNotFoundException: batch {key}"));
        &mut self.batches[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(items: &[(i32, i32)]) -> LocationDictionary<char> {
        let mut d = LocationDictionary::new();
        for &(x, y) in items {
            d.add(Location::new(x, y), 'i');
        }
        d
    }

    #[test]
    fn descending_hands_out_the_largest_key_first() {
        let mut sorted = SortedBatches::new(
            [
                (1, batch(&[(1, 1)])),
                (3, batch(&[(3, 3)])),
                (2, batch(&[(2, 2)])),
            ],
            false,
        );
        assert_eq!(
            sorted.peek().unwrap().keys().next(),
            Some(Location::new(3, 3))
        );
        sorted.move_candidate(Location::new(2, 2), 'm', 2, 3);
        assert_eq!(sorted.peek().unwrap().len(), 2);
        sorted.move_candidate(Location::new(3, 3), 'n', 3, 9);
        assert_eq!(sorted.lookup(9).unwrap().len(), 1);
        // The new batch for key 9 was never enqueued.
        assert_eq!(sorted.queue_len(), 3);
        sorted.remove_candidate(Location::new(1, 1), 1);
        assert!(sorted.lookup(1).unwrap().is_empty());
    }
}
