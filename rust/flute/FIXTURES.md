# flute fixtures

`fixtures/flute.jsonl` holds what FluteSharp computes, so the Rust port can be checked without .NET (#138). CI reruns the recorder on the pinned SDK and fails if the committed file drifts.

**Regenerate** (from the repo root, with submodules checked out):

```bash
./docker-build.sh run --project rust/flute/oracle -c Release -- rust/flute/fixtures
```

The output is deterministic: two runs give the same bytes.

## Format

JSON Lines, one case per line:

```json
{"name": "small-0005", "points": [[x, y], ...], "deg": 12, "length": 87, "branches": [[x, y, n], ...]}
```

`deg`, `length` and `branches` are the `Tree` that `new FLUTE(LookUpTable.Degree6).Execute(points)` returns, with the default accuracy of 3. `n` is the index of the branch each branch connects to. When FluteSharp throws, the case has `"error": "<exception type name>"` in place of the tree. The only such case is `empty`, with no points, which throws `OverflowException`. The Rust port returns `FluteError::NoPoints` there.

## Cases (1,346)

| Cases | Input |
|---|---|
| `small-NNNN` | All 61 blueprints in `test/FactorioTools.Test/OilField/small-list.txt` |
| `big-NNNN` | Every second blueprint in `big-list.txt` (574 of 1,147) |
| `tiny-*`, `small-random-*` | 1 to 10 random points, coordinates 0 to 4 (many duplicates) and 0 to 20 |
| `medium-random-*`, `crowded-*` | 11 to 60 random points, coordinates -10 to 40, and 0 to 6 (crowded) |
| `horizontal-*`, `vertical-*`, `diagonal-*`, `same-point-*`, `grid-*` | 3 to 14 points in a line, all on one point, or on a lattice |
| `empty` | No points |

For the blueprint cases, the points are the ones the planner hands FLUTE in `AddPipesConnectedCentersFLUTE.GetFluteTree`: every terminal of every pumpjack, in `CenterToTerminals` order, taken straight after `InitializeContext.Execute` with `OilFieldOptions.ForMediumElectricPole`. In a real plan the planner may have removed some terminals first, so these are the same kind of input, not the exact calls. Most have more than 6 points, so they run `flutes_MD` and its `float` scoring, not just the lookup table.

Random inputs come from `new Random(42)`.
