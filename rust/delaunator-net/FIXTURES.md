# delaunator-net fixtures

`fixtures/delaunator.jsonl` holds triangulations recorded from DelaunatorSharp by a recorder
in `oracle/`, so `tests/fixtures.rs` can check the Rust port without .NET (#138). The random
cases used `new Random(1)`.

**These fixtures are frozen inputs.** They were recorded from the C# planner's copy of
DelaunatorSharp at commit `0532d923`. Stage 13b (#138) deleted the C# and the recorder with
it, so nothing can regenerate them now. To see the recorder, check out that commit.

## Format

JSON Lines, one case per line:

```json
{"name": "small-0005-centers", "points": [[x, y], ...], "triangles": [...], "halfedges": [...]}
{"name": "too-few-2", "points": [[0, 0], [5, 5]], "error": "ArgumentOutOfRangeException"}
```

`triangles` and `halfedges` are `Delaunator.Triangles` and `Delaunator.Halfedges` exactly as
the C# returns them. `error` is the type name of the exception the constructor threw:
`ArgumentOutOfRangeException` for fewer than 3 points, or `Exception` for "No Delaunay
triangulation exists for this input." (all points collinear or identical).

## Cases (2,889)

| Name | What |
|---|---|
| `small-NNNN-centers`, `big-NNNN-centers` | The pumpjack centers of every blueprint in `small-list.txt` and `big-list.txt`, in `context.Centers` order after `InitializeContext.Execute` with `OilFieldOptions.ForMediumElectricPole`. The connected-centers strategies (`AddPipes.3.ConnectedCenters.DT.cs`) and FBE (`PointsToLines(context.Centers, sort: false)`) triangulate exactly this array. |
| `small-NNNN-sorted`, `big-NNNN-sorted` | The same centers sorted by X then Y, as `Helpers.PointsToLines(..., sort: true)` sorts them. Only when `PointsToLines` would reach Delaunator: more than 2 points, not collinear. |
| `too-few-*`, `collinear-*`, `identical-5`, `duplicates-square` | The error paths and duplicate handling. |
| `right-triangle-circumcenter-is-a-point` | A point sitting on the seed circumcenter, where `HashKey` computes `0 / 0`. |
| `circle-r5`, `circle-r25`, `circle-r65` | Every integer point on one circle: all cocircular. |
| `lattice-*` | Square and rectangular integer grids, spacing 1 and 3, and shuffled copies. Grids are where ties are densest. |
| `random-range*-*` | 400 random integer sets of 3 to 60 points, in ranges 6, 12, 40 and 100. |
