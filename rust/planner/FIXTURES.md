# planner fixtures

Recorded from the C# planner by `oracle/`, so the Rust tests can check the port without .NET.
Each stage of the port (#138) adds files here. CI reruns the recorder on the pinned SDK and
fails if the committed files drift.

**Regenerate** (from the repo root, with submodules checked out):

```bash
./docker-build.sh run --project rust/planner/oracle -c Release -- rust/planner/fixtures
```

All files are JSON Lines, one case per line. The output is deterministic. One run writes every file.

## `grids.jsonl` (stage 1)

The final grid of a real plan, and the text `SquareGrid.ToString(builder, spacing: 1)`
draws for it. That text is the grid half of every Verify planner snapshot, so matching it
here means the Rust grid, its entities and their labels draw the snapshots' grids exactly.

The cases (142 in all) are option sets copied from the `PlannerOnAllBlueprintsAndOptions`
theory, so each has a committed Verify snapshot, plus heat pipes and four `PlannerTest` cases:

| Cases | Blueprints | Options |
|---|---|---|
| `small-NNNN_medium-electric-pole_b_o_u` | all 61 in `small-list.txt` | medium pole, beacons, overlap, underground |
| `small-NNNN_small-electric-pole_u` | every third, from index 0 | small pole, no beacons, underground |
| `small-NNNN_big-electric-pole_b` | every third, from index 1 | big pole, beacons without overlap, no underground |
| `small-NNNN_substation_b_o` | every third, from index 2 | substation, beacons, overlap, no underground |
| `small-NNNN_medium-electric-pole_heat` | every fourth, from index 0 | medium pole, heat pipes, no beacons |
| `PlannerTest.ExecuteSample`, `.AddsHeatPipesForAquilo`, `.AddsHeatPipesAndBeaconsTogetherForAquilo`, `.AllowsLocationsToBeAvoided` | as in `PlannerTest.cs` | as in `PlannerTest.cs` |

The first four sets use `ValidateSolution = true`, as the theory does. Every entity kind that a
finished plan can hold appears. `TemporaryEntity` does not: the planner removes them all before
it returns. `AvoidEntity` comes only from the avoid-location case.

```json
{
  "name": "small-0005_medium-electric-pole_b_o_u",
  "width": 40, "height": 31,
  "middle": [20, 15],
  "entities": [ [x, y, "Kind", id, ...extra], ... ],
  "text": "<the exact ToString output>"
}
```

`middle` is `SquareGrid.Middle`, which is `(Width / 2, Height / 2)`. `entities` lists every grid
entity in `SquareGrid.EntityLocations` enumeration order. `Kind` is the C# class name, and
`id` is `GridEntity.Id`. The extra fields depend on the kind:

| Kind | Extra fields |
|---|---|
| `BeaconCenter`, `Pipe`, `Terminal`, `HeatPipe`, `TemporaryEntity`, `AvoidEntity` | none. `Terminal` (and `UndergroundPipe`) subclass `Pipe` and add nothing else |
| `PumpjackCenter` | its `Direction`, as the C# `Direction` enum's integer value |
| `PumpjackSide`, `BeaconSide`, `ElectricPoleSide` | the id of the center entity |
| `ElectricPoleCenter` | the neighbor ids as one array, in `Neighbors` enumeration order |
| `UndergroundPipe` | its `Direction`, as the C# `Direction` enum's integer value |

`Direction` values are `Up = 0`, `Right = 2`, `Down = 4`, `Left = 6`
(`src/FactorioTools/Data/Direction.cs`). The labels `ToString` draws are `J`/`j` pumpjack
center/side, `B`/`b` beacon, `E`/`e` electric pole, `o` pipe, `+` terminal, `h` heat pipe,
`?` temporary, `X` avoid, `.` empty, and `^ > v <` for underground pipes by direction.

`text` is exactly what `ToString` builds: rows from the smallest to the largest entity Y,
columns from the smallest to the largest entity X, each cell's label padded on the right to
the longest label plus one space, so every row ends with a space. **Rows end with `\n`.**
`StringBuilder.AppendLine` writes `Environment.NewLine`, so the recorder turns any `\r\n` into
`\n` to keep the fixture the same on every OS. The Verify snapshots are LF too.

The Rust test rebuilds each grid by adding the entities in this order, then checks that
`EntityLocations` enumerates in the same order and that its `to_string` equals `text`
byte for byte.

**Checking the recorder itself:** add `--check-snapshots` to the command above. The recorder
then also compares each case's `text` with the grid half of its committed Verify snapshot (the
part after the first blank line) and exits 1 on any difference. On 2026-10-21 all 126
snapshot-backed cases matched.

## `algorithms.jsonl` (stage 2)

Calls to the planner's graph and line algorithms in `src/FactorioTools/OilField/Algorithms`,
made by the recorder through their public methods (no C# is changed to record them), with
every result. Written by `oracle/Algorithms.cs`. The inputs are realistic: for each of the 61
`small-list.txt` blueprints and every 20th `big-list.txt` blueprint, the recorder builds the
planner's starting `Context` (`InitializeContext.Execute` with
`OilFieldOptions.ForMediumElectricPole`) and calls the algorithms between its pumpjack
terminals, picked with a fixed-seed `Random`. For the small list it also plans the blueprint
and walks the finished pipes through an `ExistingPipeGrid`, as `RotateOptimize` and `Validate`
do.

Points are written as one flat array, `[x0, y0, x1, y1, ...]`, and every set or dictionary in
the order the C# enumerates it. Lines come in these kinds, in file order:

| `kind` | Fields |
|---|---|
| `grid` | `id`, `width`, `height`, `occupied`: the starting grid's entity cells, in `EntityLocations` order. The Rust test places a `TemporaryEntity` on each, since `PipeGrid.GetNeighbors` only asks whether a cell is empty. Later lines name it in `grid`. |
| `pipes` | `id`, `width`, `height`, `pipes`: a finished plan's `Pipe` cells (including terminals and underground pipes), as a `LocationHashSet`. Later lines name it in `pipes` and walk it as an `ExistingPipeGrid`. |
| `astar` | `grid` or `pipes`, `start`, `goals`, `goalsKind` (`hash` for a `LocationHashSet`, `single` for a `SingleLocationSet`), `preferNoTurns`, `xWeight`, `yWeight`, `prefix` (the caller's `outputList` before the call, or null for none), then `success`, `reached` and `path` (null when no goal was reached; it includes `prefix` when there is one). |
| `dijkstra` | `grid` or `pipes`, `start`, `goals`, `stopOnFirstGoal`, `allowGoalEnumerate`, then `reachedCount` and either `reached` (in order) or `reachedContains` (one flag per goal, since a `LocationBitSet` cannot be enumerated), then `previousCount`, `previousHash` and, for the first call per blueprint, `previous`. |
| `bfs` | `grid`, `start`, `goal`, `path` (start first, or null). |
| `prims` | `graph` (each node with its `[x, y, cost, ...]` edges, both in order), `first`, `digraph`, `mst` (each node with its neighbors, both in order). The graphs are the Delaunay graph `ExecuteWithDelaunayMst` builds (the recorder checks its copy against the planner's own result) and a complete graph over up to 8 centers with costs 1 to 3, so ties are common. |
| `bresenham` | `a`, `b`, `path`. |

**`LocationToPrevious` is recorded indirectly.** Its values are `LocationBitSet`s, which
throw when enumerated. Every previous cell is a grid neighbor, so the recorder probes the four
in-bounds neighbors of each key (east, north, west, south as bits 0 to 3) and checks the mask
accounts for the set's whole `Count`. `previous` is `[x, y, mask, ...]` in key order, and
`previousHash` is 64-bit FNV-1a (offset 14695981039346656037) over the little-endian bytes of
those ints, so most calls can be checked without writing every cell.

On 2026-10-21 the file held 1,672 A* calls (244 of them on an `ExistingPipeGrid`), 479 Dijkstra
calls (122 on an `ExistingPipeGrid`), 714 BFS calls, 474 Prim's calls and 400 Bresenham lines,
in 3.1 MB.

## `initialize_context.jsonl` (stage 3)

`InitializeContext.Execute(options, blueprint, avoid, minWidth, minHeight)`: the input and the
whole `Context` it builds, or the `FactorioToolsException` it throws. The recorder is
`oracle/InitializeContextRecorder.cs`.

Lines come in two kinds, told apart by `kind`:

- **`"preset"`** (6 lines, first): `new OilFieldOptions()` and every `OilFieldOptions.For*`
  preset, as `{"kind", "name", "options"}`. The Rust test checks its own defaults and presets
  against these field by field.
- **`"case"`**: one `Execute` call.

| Cases | Blueprints | Options |
|---|---|---|
| `small-NNNN_medium` | all 61 in `small-list.txt` | `ForMediumElectricPole` |
| `small-NNNN_small`, `_big`, `_substation` | every third, from index 0, 1 and 2 | the matching preset |
| `small-NNNN_medium-no-beacons`, `_medium-non-standard-beacon` | every third, from index 1 and 2 | no beacons; a 2x4 beacon with a 4x6 supply area |
| `small-NNNN_medium-legendary-pole`, `_big-rare-everything` | every sixth, from index 0 and 3 | electric pole quality, plus the output-only qualities |
| `big-NNNN_medium` | every 20th in `big-list.txt` | `ForMediumElectricPole` |
| `test-*`, `avoid-only`, `planner-test-avoid` | the inputs from `InitializeContextTest` and `PlannerTest.AllowsLocationsToBeAvoided`, plus odd min-size padding and an avoid-only blueprint | as in those tests |
| `mixed-entities-and-directions` | pumpjacks facing every way, one with no direction, one mirrored, plus a pipe and a beacon (ignored) | `ForMediumElectricPole` |
| `error-*` | one for each exception `Execute` can throw: more than 150 pumpjacks, a non-whole grid width or height, a non-whole pumpjack or avoid X or Y, a grid over 1000 wide or over 250,000 cells, an avoid on a pumpjack, the same avoid twice, overlapping pumpjacks, and a pumpjack with no free terminal | `ForMediumElectricPole` |

A case line:

```json
{
  "kind": "case", "name": "small-0005_medium",
  "options": { ...every OilFieldOptions property... },
  "entities": [ [entity_number, name, x_bits, y_bits, direction or null, mirror or null], ... ],
  "avoid": [ [x_bits, y_bits], ... ],
  "min": [minWidth, minHeight],
  "context": { ... }            // or "error": {"message": "...", "bad_input": false}
}
```

**Floats are written as their bits** (`BitConverter.SingleToInt32Bits`, and
`DoubleToInt64Bits` for doubles, in fields ending `_bits`), so they read back exactly. The
options use the C# property names in snake case; strategies and qualities are their enum
integers, and module dictionaries are `[name, count]` pairs in dictionary order.

`context` holds:

| Field | C# |
|---|---|
| `delta_x_bits`, `delta_y_bits` | `DeltaX`, `DeltaY` (`float`) |
| `width`, `height`, `middle` | the grid's |
| `entities` | every grid entity in `EntityLocations` order, encoded as in `grids.jsonl`. Only `PumpjackCenter` (with its direction, always `Up` at this point), `PumpjackSide` and `AvoidEntity` appear |
| `next_id` | what the grid's next `GetId()` returns |
| `centers` | `Centers`, in order (sorted by Y, then X) |
| `center_to_terminals`, `location_to_terminals` | `[x, y, [[center_x, center_y, terminal_x, terminal_y, direction], ...]]` per key, in dictionary order |
| `center_to_original_direction` | `[x, y, direction]` per key, in dictionary order |
| `adjacent_count_length`, `adjacent_counts` | `LocationToAdjacentCount`: its length, then `[index, count]` for each non-zero cell |
| `electric_pole_*_with_quality` | the four quality-scaled pole fields |
