//! `ArraySortHelper<T>.IntrospectiveSort` (.NET 10), the sort behind `List<T>.Sort`,
//! `Array.Sort` and `MemoryExtensions.Sort` when given a comparison.
//!
//! It is not stable. Items the comparison calls equal end up in an order that depends on
//! the algorithm's exact steps, so the port copies them: median-of-three partitioning,
//! insertion sort for 16 items or fewer, and heapsort past the depth limit. Sorts whose
//! comparison never returns 0 for different items give the same result with any sort.

use std::cmp::Ordering;

/// `Array.IntrosortSizeThreshold`.
const INTROSORT_SIZE_THRESHOLD: usize = 16;

/// Sorts the whole slice like `List<T>.Sort(Comparison<T>)`.
pub fn net_sort<T, F>(items: &mut [T], mut compare: F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    introspective_sort(items, &mut compare);
}

/// Sorts `items[index..index + count]` like `List<T>.Sort(index, count, comparer)`.
pub fn net_sort_range<T, F>(items: &mut [T], index: usize, count: usize, mut compare: F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    introspective_sort(&mut items[index..index + count], &mut compare);
}

/// `IntrospectiveSort(Span<T> keys, Comparison<T> comparer)`.
fn introspective_sort<T, F>(keys: &mut [T], compare: &mut F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    if keys.len() > 1 {
        // 2 * (BitOperations.Log2((uint)keys.Length) + 1)
        let depth_limit = 2 * (keys.len().ilog2() as usize + 1);
        intro_sort(keys, depth_limit, compare);
    }
}

/// `IntroSort`: partition, recurse into the right part, loop on the left part.
fn intro_sort<T, F>(keys: &mut [T], mut depth_limit: usize, compare: &mut F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    let mut partition_size = keys.len();
    while partition_size > 1 {
        if partition_size <= INTROSORT_SIZE_THRESHOLD {
            if partition_size == 2 {
                swap_if_greater(keys, compare, 0, 1);
                return;
            }
            if partition_size == 3 {
                swap_if_greater(keys, compare, 0, 1);
                swap_if_greater(keys, compare, 0, 2);
                swap_if_greater(keys, compare, 1, 2);
                return;
            }
            insertion_sort(&mut keys[..partition_size], compare);
            return;
        }

        if depth_limit == 0 {
            heap_sort(&mut keys[..partition_size], compare);
            return;
        }
        depth_limit -= 1;

        let p = pick_pivot_and_partition(&mut keys[..partition_size], compare);

        // The pivot is already in place, so neither side includes it.
        intro_sort(&mut keys[p + 1..partition_size], depth_limit, compare);
        partition_size = p;
    }
}

/// `SwapIfGreater`: swap only when the first is strictly greater.
fn swap_if_greater<T, F>(keys: &mut [T], compare: &mut F, i: usize, j: usize)
where
    F: FnMut(&T, &T) -> Ordering,
{
    if compare(&keys[i], &keys[j]) == Ordering::Greater {
        keys.swap(i, j);
    }
}

/// `PickPivotAndPartition`: median of three, pivot parked at `hi - 1`, then a Hoare-style
/// partition that stops on items equal to the pivot from both sides.
fn pick_pivot_and_partition<T, F>(keys: &mut [T], compare: &mut F) -> usize
where
    F: FnMut(&T, &T) -> Ordering,
{
    let hi = keys.len() - 1;

    // Compute median-of-three, and order those three while at it.
    let middle = hi >> 1;
    swap_if_greater(keys, compare, 0, middle);
    swap_if_greater(keys, compare, 0, hi);
    swap_if_greater(keys, compare, middle, hi);

    keys.swap(middle, hi - 1);
    let pivot = hi - 1;
    let mut left = 0;
    let mut right = hi - 1;

    while left < right {
        // while (comparer(keys[++left], pivot) < 0) ;
        loop {
            left += 1;
            if compare(&keys[left], &keys[pivot]) != Ordering::Less {
                break;
            }
        }
        // while (comparer(pivot, keys[--right]) < 0) ;
        loop {
            right -= 1;
            if compare(&keys[pivot], &keys[right]) != Ordering::Less {
                break;
            }
        }

        if left >= right {
            break;
        }
        keys.swap(left, right);
    }

    // Put the pivot in its final place.
    if left != hi - 1 {
        keys.swap(left, hi - 1);
    }
    left
}

/// `HeapSort`, with .NET's 1-based `DownHeap`.
fn heap_sort<T, F>(keys: &mut [T], compare: &mut F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    let n = keys.len();
    let mut i = n >> 1;
    while i >= 1 {
        down_heap(keys, i, n, compare);
        i -= 1;
    }

    let mut i = n;
    while i > 1 {
        keys.swap(0, i - 1);
        down_heap(keys, 1, i - 1, compare);
        i -= 1;
    }
}

/// `DownHeap(keys, i, n, comparer)`. .NET holds the moving item in a local and shifts
/// children up into the hole. Swapping along the same path ends with the same layout,
/// since every comparison is against that one item.
fn down_heap<T, F>(keys: &mut [T], mut i: usize, n: usize, compare: &mut F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    // `d` is at keys[i - 1] throughout, because each step swaps it down.
    while i <= n >> 1 {
        let mut child = 2 * i;
        if child < n && compare(&keys[child - 1], &keys[child]) == Ordering::Less {
            child += 1;
        }

        if compare(&keys[i - 1], &keys[child - 1]) != Ordering::Less {
            break;
        }

        keys.swap(i - 1, child - 1);
        i = child;
    }
}

/// `InsertionSort`: shift while the new item is strictly smaller, so equal items keep
/// their order here.
fn insertion_sort<T, F>(keys: &mut [T], compare: &mut F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    for i in 0..keys.len().saturating_sub(1) {
        let mut j = i + 1;
        // keys[j] holds t; move it left past every strictly greater item.
        while j > 0 && compare(&keys[j], &keys[j - 1]) == Ordering::Less {
            keys.swap(j, j - 1);
            j -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts() {
        let mut v: Vec<i32> = (0..200).map(|i| (i * 7919) % 101).collect();
        net_sort(&mut v, |a, b| a.cmp(b));
        assert!(v.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn small_sorts_keep_equal_items_in_order() {
        let mut v = vec![(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd')];
        net_sort(&mut v, |a, b| a.0.cmp(&b.0));
        assert_eq!(v, vec![(0, 'b'), (0, 'd'), (1, 'a'), (1, 'c')]);
    }
}
