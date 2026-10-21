//! Rust copies of the .NET behavior the oil-field planner's output depends on (#138).
//!
//! The C# planner often takes "the first item" of a set, breaks ties by heap position, or
//! sorts with a comparison that allows ties. In all of those the result depends on exactly
//! how .NET 10's collections and sort work, so the Rust port uses these types wherever
//! order matters. Each type is tested against output recorded from real .NET (see
//! `FIXTURES.md` and `tests/fixtures.rs`).
//!
//! - [`NetHashSet`] and [`NetDictionary`]: `System.Collections.Generic.HashSet<T>` and
//!   `Dictionary<TKey, TValue>`, which enumerate in entry order and reuse freed entries
//!   last-in first-out.
//! - [`NetPriorityQueue`]: `PriorityQueue<TElement, TPriority>`, a 4-ary min-heap.
//! - [`net_sort`] and [`net_sort_range`]: `ArraySortHelper<T>.IntrospectiveSort`, the
//!   unstable sort behind `List<T>.Sort`, `Array.Sort` and `MemoryExtensions.Sort`.
//! - [`round_half_even`] and [`round_to_i32`]: `Math.Round(x, 0)` and `(int)` conversions.

mod compare;
mod hash_helpers;
mod hash_table;
mod math;
mod priority_queue;
mod sort;

pub use compare::NetCompare;
pub use hash_helpers::{expand_prime, get_prime};
pub use hash_table::{NetDictionary, NetHashSet};
pub use math::{round_half_even, round_to_i32, truncate_to_i32};
pub use priority_queue::NetPriorityQueue;
pub use sort::{net_sort, net_sort_range};
