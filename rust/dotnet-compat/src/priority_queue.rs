//! `PriorityQueue<TElement, TPriority>` from System.Collections.Generic (.NET 10).
//!
//! A 4-ary min-heap in an array. Ties between equal priorities are broken by where the
//! nodes happen to sit in the heap, so a different heap (std's `BinaryHeap`, say) dequeues
//! equal priorities in a different order, and the planner would pick different paths.

use std::cmp::Ordering;

use crate::compare::NetCompare;

/// `Arity` (and `Log2Arity` = 2).
const ARITY: usize = 4;

/// A copy of .NET's `PriorityQueue<TElement, TPriority>` with the default comparer.
#[derive(Clone, Debug)]
pub struct NetPriorityQueue<E, P> {
    /// `_nodes[0.._size]`.
    nodes: Vec<(E, P)>,
}

impl<E, P> Default for NetPriorityQueue<E, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, P> NetPriorityQueue<E, P> {
    pub fn new() -> Self {
        NetPriorityQueue { nodes: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        NetPriorityQueue {
            nodes: Vec::with_capacity(capacity),
        }
    }

    /// `Count`.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// `Clear`.
    pub fn clear(&mut self) {
        self.nodes.clear();
    }
}

impl<E, P: NetCompare> NetPriorityQueue<E, P> {
    /// `Enqueue`.
    pub fn enqueue(&mut self, element: E, priority: P) {
        let index = self.nodes.len();
        self.nodes.push((element, priority));
        self.move_up(index);
    }

    /// `Peek`: the element with the lowest priority, without removing it.
    pub fn peek(&self) -> Option<&E> {
        self.nodes.first().map(|(e, _)| e)
    }

    /// `TryPeek`.
    pub fn peek_with_priority(&self) -> Option<(&E, &P)> {
        self.nodes.first().map(|(e, p)| (e, p))
    }

    /// `Dequeue`. Returns `None` where .NET would throw.
    pub fn dequeue(&mut self) -> Option<E> {
        self.try_dequeue().map(|(e, _)| e)
    }

    /// `TryDequeue`.
    pub fn try_dequeue(&mut self) -> Option<(E, P)> {
        if self.nodes.is_empty() {
            return None;
        }
        Some(self.remove_root_node())
    }

    /// `RemoveRootNode`: take the root, then sift the last node down from the root.
    fn remove_root_node(&mut self) -> (E, P) {
        let last = self.nodes.len() - 1;
        self.nodes.swap(0, last);
        let root = self.nodes.pop().expect("queue is not empty");
        if !self.nodes.is_empty() {
            self.move_down(0);
        }
        root
    }

    /// `Remove(element, out removedElement, out priority)` with the default equality
    /// comparer. It finds the first match in array order. If that is not the last node, the
    /// last node moves into its place, then sifts up if it is strictly smaller than its new
    /// parent and down otherwise. (A down-only version fails the recorded fixtures: .NET 10
    /// keeps the heap valid after a removal from the middle.)
    pub fn remove(&mut self, element: &E) -> Option<(E, P)>
    where
        E: PartialEq,
    {
        let index = self.nodes.iter().position(|(e, _)| e == element)?;
        let new_size = self.nodes.len() - 1;
        if index < new_size {
            self.nodes.swap(index, new_size);
            let removed = self.nodes.pop().expect("queue is not empty");
            let parent = index.checked_sub(1).map(|i| i >> 2);
            match parent {
                Some(parent)
                    if self.nodes[index].1.net_cmp(&self.nodes[parent].1) == Ordering::Less =>
                {
                    self.move_up(index)
                }
                _ => self.move_down(index),
            }
            Some(removed)
        } else {
            self.nodes.pop()
        }
    }

    /// The nodes in heap-array order, as `UnorderedItems` enumerates them.
    pub fn unordered_items(&self) -> impl Iterator<Item = &(E, P)> {
        self.nodes.iter()
    }

    /// `MoveUpDefaultComparer`: the node at `index` moves up while it is strictly smaller
    /// than its parent.
    fn move_up(&mut self, mut index: usize) {
        while index > 0 {
            let parent = (index - 1) >> 2;
            if self.nodes[index].1.net_cmp(&self.nodes[parent].1) == Ordering::Less {
                self.nodes.swap(index, parent);
                index = parent;
            } else {
                break;
            }
        }
    }

    /// `MoveDownDefaultComparer`: the node at `index` moves down to its smallest child (the
    /// first one among equals) while it is strictly greater than that child.
    fn move_down(&mut self, mut index: usize) {
        let size = self.nodes.len();
        loop {
            let first_child = (index << 2) + 1;
            if first_child >= size {
                break;
            }
            let mut min_child = first_child;
            let upper = (first_child + ARITY).min(size);
            for i in first_child + 1..upper {
                if self.nodes[i].1.net_cmp(&self.nodes[min_child].1) == Ordering::Less {
                    min_child = i;
                }
            }
            if self.nodes[index].1.net_cmp(&self.nodes[min_child].1) != Ordering::Greater {
                break;
            }
            self.nodes.swap(index, min_child);
            index = min_child;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_priorities_follow_heap_position() {
        let mut queue = NetPriorityQueue::new();
        for e in 0..6 {
            queue.enqueue(e, 1.0);
        }
        // Dequeuing 0 moves 5 (the last node) to the root; children 1-4 tie with it, so it
        // stays there and comes out next.
        assert_eq!(queue.dequeue(), Some(0));
        assert_eq!(queue.dequeue(), Some(5));
    }
}
