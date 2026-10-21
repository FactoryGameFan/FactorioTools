# planner fixtures

Recorded from the C# planner by `oracle/`, so the Rust tests can check the port without .NET.
Each stage of the port (#138) adds files here. CI reruns the recorder on the pinned SDK and
fails if the committed files drift.

**Regenerate** (from the repo root, with submodules checked out):

```bash
./docker-build.sh run --project rust/planner/oracle -c Release -- rust/planner/fixtures
```

All files are JSON Lines, one case per line. The output is deterministic.

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
