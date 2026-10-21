# delaunator-net

A Rust port of the parts of [DelaunatorSharp](https://github.com/nol1fe/delaunator-sharp) by
Patryk Grech that the oil-field planner uses. DelaunatorSharp is itself a C# port of
[Mapbox's Delaunator](https://github.com/mapbox/delaunator). Both are credited here, and the
MIT license in `LICENSE` is DelaunatorSharp's, copied unchanged.

It copies the C# line for line rather than using the `delaunator` crate. The planner's points
lie on a grid, so many of them sit on the same circle, and the triangle order then depends on
details such as the seed choice, the custom quicksort and the non-robust predicates. Those
details decide which pipes the planner draws (#138). `tests/fixtures.rs` checks the port
against triangulations recorded from the C# (see `FIXTURES.md`).
