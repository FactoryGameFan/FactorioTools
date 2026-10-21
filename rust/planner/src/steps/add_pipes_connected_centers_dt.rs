//! `AddPipesConnectedCentersDT`, from `OilField/Steps/AddPipes.3.ConnectedCenters.DT.cs`: the
//! pumpjack graphs the `ConnectedCentersDelaunay` and `ConnectedCentersDelaunayMst` strategies
//! start from.
//!
//! Both read the triangulation the same way the C# does: every half-edge `e` with
//! `e > halfedges[e]`, which takes each inner edge once and every hull edge (whose half-edge
//! is -1) once, in half-edge order. That order decides the order of every set in the graph,
//! so it uses `delaunator-net`, which matches DelaunatorSharp exactly.

use delaunator_net::Delaunator;

use crate::Location;
use crate::algorithms::get_minimum_spanning_tree;
use crate::collection_ext::min_by;
use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;

/// `ExecuteWithDelaunay`: each pumpjack is connected to its Delaunay neighbors.
pub fn execute_with_delaunay(
    context: &Context,
    centers: &[Location],
) -> LocationDictionary<LocationSet> {
    let delaunator = get_delaunator(centers);

    // `centers.ToDictionary(context, c => c, c => context.GetLocationSet(allowEnumerate: true))`.
    let mut dl_graph = context.get_location_dictionary_with_capacity(centers.len());
    for &center in centers {
        dl_graph.add(center, context.get_location_set_enumerable(true));
    }

    for_each_edge(&delaunator, centers, |p, q| {
        dl_graph.value_mut(p).add(q);
        dl_graph.value_mut(q).add(p);
    });

    dl_graph
}

/// `ExecuteWithDelaunayMst`: the Delaunay graph weighted by squared distance, reduced to
/// Prim's minimum spanning tree grown from the pumpjack closest to the grid's middle.
pub fn execute_with_delaunay_mst(
    context: &Context,
    centers: &[Location],
) -> LocationDictionary<LocationSet> {
    let delaunator = get_delaunator(centers);

    // `centers.ToDictionary(context, c => c, c => context.GetLocationDictionary<int>())`.
    let mut dl_graph: LocationDictionary<LocationDictionary<i32>> =
        context.get_location_dictionary_with_capacity(centers.len());
    for &center in centers {
        dl_graph.add(center, context.get_location_dictionary());
    }

    for_each_edge(&delaunator, centers, |p, q| {
        let cost = p.euclidean_distance_squared(q);
        // `dlGraph[p][q] = cost`: the indexer, which adds or overwrites.
        dl_graph.value_mut(p).set(q, cost);
        dl_graph.value_mut(q).set(p, cost);
    });

    // `centers.MinBy(context.Grid.Middle.GetEuclideanDistanceSquared)`: the first center
    // with the smallest squared distance.
    let middle = context.grid.middle();
    let closest_to_middle = min_by(centers.iter().copied(), |c| {
        middle.euclidean_distance_squared(*c)
    })
    .expect("MinBy over the centers, which ConnectedCenters only calls with three or more");

    get_minimum_spanning_tree(&dl_graph, closest_to_middle, false)
}

/// The C#'s `for (var e = 0; e < Triangles.Length; e++) if (e > Halfedges[e]) ...` loop.
fn for_each_edge(
    delaunator: &Delaunator,
    centers: &[Location],
    mut visit: impl FnMut(Location, Location),
) {
    for e in 0..delaunator.triangles.len() {
        if e as i32 > delaunator.halfedges[e] {
            let p = centers[delaunator.triangles[e] as usize];
            let next = if e % 3 == 2 { e - 2 } else { e + 1 };
            let q = centers[delaunator.triangles[next] as usize];
            visit(p, q);
        }
    }
}

/// `GetDelauntator` (sic): the centers as `double` points.
///
/// The C# never reaches the "no triangulation" case: `GetConnectedPumpjacks` handles two
/// centers and collinear centers before calling here. DelaunatorSharp throws a plain
/// `Exception` for it, not a `FactorioToolsException`, so the port panics.
fn get_delaunator(centers: &[Location]) -> Delaunator {
    let points: Vec<(f64, f64)> = centers
        .iter()
        .map(|c| (f64::from(c.x), f64::from(c.y)))
        .collect();
    match Delaunator::new(&points) {
        Ok(delaunator) => delaunator,
        Err(error) => panic!("Delaunator: {error:?}"),
    }
}
