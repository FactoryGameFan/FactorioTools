//! Location-keyed containers, from `src/FactorioTools/OilField/Containers`.
//!
//! `SharedInstances` (`OilField/Models/SharedInstances.cs`) is not ported: it holds scratch
//! buffers that the C# reuses to save allocations and clears after each use, so nothing
//! observable depends on it.

mod counted_bit_array;
mod location_dictionary;
mod location_set;
mod sorted_batches;

pub use counted_bit_array::CountedBitArray;
pub use location_dictionary::LocationDictionary;
pub use location_set::{LocationBitSet, LocationHashSet, LocationSet, SingleLocationSet};
pub use sorted_batches::SortedBatches;
