//! Prim's minimum spanning tree, from `Algorithms/Prims.cs`.
//!
//! Edge costs are integers and often tie, so which tree comes out depends on .NET's
//! `PriorityQueue` order and on the order the graph's dictionaries enumerate.

use dotnet_compat::NetPriorityQueue;

use super::LocationLookup;
use crate::Location;
use crate::containers::{LocationDictionary, LocationSet};

/// `Prims.GetMinimumSpanningTree(context, graph, firstNode, digraph)`.
///
/// The result maps each node to the nodes it connects to, in .NET's order. When `digraph`
/// is false every edge is added in both directions.
pub fn get_minimum_spanning_tree(
    graph: &LocationDictionary<LocationDictionary<i32>>,
    first_node: Location,
    digraph: bool,
) -> LocationDictionary<LocationSet> {
    let mut priority: NetPriorityQueue<(Location, Location), i32> = NetPriorityQueue::new();
    let mut mst: LocationDictionary<LocationSet> = LocationDictionary::new();
    let mut visited = LocationLookup::default();

    visited.insert(first_node);
    for (other_node, &cost) in graph.value(first_node).iter() {
        priority.enqueue((first_node, other_node), cost);
    }

    while let Some((node_a, node_b)) = priority.dequeue() {
        if !visited.insert(node_b) {
            continue;
        }

        match mst.get_mut(node_a) {
            Some(nodes) => {
                nodes.add(node_b);
            }
            None => {
                // `context.GetLocationSet(nodeB, allowEnumerate: true)`.
                let mut nodes = LocationSet::new_hash();
                nodes.add(node_b);
                mst.add(node_a, nodes);
            }
        }

        for (neighbor, &cost) in graph.value(node_b).iter() {
            if !visited.contains(&neighbor) {
                priority.enqueue((node_b, neighbor), cost);
            }
        }
    }

    if !digraph {
        // Make the MST bidirectional (a graph, not a digraph). `mst.Keys.ToList()` takes the
        // keys before any are added; each set is read before the loop changes other entries.
        let centers: Vec<Location> = mst.keys().collect();
        for center in centers {
            let neighbors = mst.value(center).to_vec();
            for neighbor in neighbors {
                match mst.get_mut(neighbor) {
                    Some(other_neighbors) => {
                        other_neighbors.add(center);
                    }
                    None => {
                        let mut other_neighbors = LocationSet::new_hash();
                        other_neighbors.add(center);
                        mst.add(neighbor, other_neighbors);
                    }
                }
            }
        }
    }

    mst
}
