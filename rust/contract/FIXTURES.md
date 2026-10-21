# contract fixtures

Recorded from the C# contract by `oracle/`, so the Rust tests can check `rust/contract` (and
through it `rust/blueprint`'s writer) without .NET (#138). CI reruns the recorder on the
pinned SDK and fails if the committed file drifts.

**Regenerate** (from the repo root, with submodules checked out):

```bash
./docker-build.sh run --project rust/contract/oracle -c Release -- rust/contract/fixtures
```

The output is deterministic.

## `contract.jsonl`

One line per call. Most are a call of `PlanOrchestrator.PlanJson` or `NormalizeJson`:

```json
{ "name": "small-0005_heat", "op": "plan" | "normalize", "request": "<the request JSON text>",
  "outcome": "ok", "response": "<the response JSON text>" }
{ "name": "...", "op": "...", "request": "...",
  "outcome": "threw", "exception": "JsonException", "message": "..." }
```

`"ok"` includes the error envelope, which the C# returns like any response. `"threw"` is
an exception the C# lets escape the call: a `JsonException` for malformed or mistyped JSON,
or a `NullReferenceException` for a `null` request.

The requests cover:

- every small-list blueprint with only `blueprint` given, so every option takes its default;
- full option objects, the way the site sends them: each pole preset, beacons and
  underground off, beacon overlap off, heat pipes with and without beacons, every quality
  setting, the FBE offset, validation, other modules, every strategy, poles and the pipe
  optimizer off;
- every 50th big-list blueprint;
- how requests are read: names in any case, numbers in strings, enum names in lower case
  and as integers, an undefined quality integer, unknown and repeated properties, a fraction
  for the wire reach, escaped characters;
- `NormalizeJson` on every small-list blueprint and every 25th big-list blueprint;
- bad blueprints (empty, wrong version byte, bad base64, not zlib, not JSON, `null`, no
  blueprint, a book, no pumpjacks, a diagonal pumpjack) and bad requests (malformed JSON,
  `null`, an array, no blueprint, a `null` blueprint, a string for a bool, an unknown
  quality, a fraction for an int, a trailing comma, no pipe strategies), for both calls.

Six more lines record `GridToBlueprintString.Execute` with `addAvoidEntities: true`, which
only the CLI uses: a small-list blueprint planned with two avoided cells, and the decoded
JSON of the blueprint string it writes.

```json
{ "name": "grid_small-0000_avoid", "op": "grid_to_blueprint_string", "blueprint": "...",
  "avoid": [[x, y], ...], "add_fbe_offset": false, "outcome": "ok", "json": "..." }
```

## What the Rust test compares

- A response must match the recorded text exactly, except blueprint strings, which are
  compared after decompressing. zlib's bytes differ between .NET and Rust, but the JSON
  inside must be the same text.
- An error envelope must match exactly, except where `System.Text.Json`'s reader wrote the
  inner messages (a blueprint whose JSON does not parse). There the title, status and outer
  message must match; the Rust port gives serde_json's description as the inner message.
- A `"threw"` case must fail in Rust with the same exception type. The messages are
  `System.Text.Json`'s and are not compared.
