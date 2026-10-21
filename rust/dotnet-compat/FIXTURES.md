# dotnet-compat fixtures

The planner's output depends on how .NET orders sets, dictionaries, priority queues and
sorts when there are ties (#138). This crate copies that behavior. The fixtures here are
recorded from real .NET by `oracle/`, so the Rust tests can check the copy without .NET.

**Regenerate** (needs the .NET SDK, or `./docker-build.sh` from the repo root):

```bash
./docker-build.sh run --project rust/dotnet-compat/oracle -c Release -- rust/dotnet-compat/fixtures
```

The output is deterministic: every scenario comes from a fixed seed, so a rerun on the
same SDK gives the same bytes. CI reruns it and fails if the committed files drift.

## Format

Each file is JSON Lines: one scenario per line, `{"name": "...", "ops": [...]}`. Each op
is an array whose first item is the op name. Ops that return something carry the
expected result as their last item, which the Rust test compares exactly.

### `hashset.jsonl` - `HashSet<int>`

| Op | Meaning | Expected |
|---|---|---|
| `["new", capacity]` | `new HashSet<int>(capacity)`; capacity -1 means `new HashSet<int>()` | - |
| `["add", k, bool]` | `Add(k)` | its return value |
| `["remove", k, bool]` | `Remove(k)` | its return value |
| `["contains", k, bool]` | `Contains(k)` | its return value |
| `["clear"]` | `Clear()` | - |
| `["union", [ks]]` | `UnionWith(ks)` with a `List<int>` | - |
| `["union_set", [ks]]` | `UnionWith(new HashSet<int>(ks))` | - |
| `["except", [ks]]` | `ExceptWith(ks)` with a `List<int>` | - |
| `["intersect", [ks]]` | `IntersectWith(ks)` with a `List<int>` | - |
| `["intersect_set", [ks]]` | `IntersectWith(new HashSet<int>(ks))` | - |
| `["overlaps", [ks], bool]` | `Overlaps(ks)` | its return value |
| `["set_equals", [ks], bool]` | `SetEquals(ks)` | its return value |
| `["copy"]` | replace the set with `new HashSet<int>(set)`, the copy constructor from a `HashSet` with the same comparer. That constructor can keep freed slots, depending on capacity. | - |
| `["copy_list"]` | replace the set with `new HashSet<int>(set.ToList())` | - |
| `["ensure_capacity", n, int]` | `EnsureCapacity(n)` | its return value |
| `["trim"]` | `TrimExcess()` | - |
| `["order", [ks]]` | enumerate the set | the keys in enumeration order |

Every scenario ends with an `order` op, and most have several in between.

### `dictionary.jsonl` - `Dictionary<int, int>`

`new`, `remove`, `contains` (as `ContainsKey`), `clear`, `copy`, `ensure_capacity`, `trim`
and `order` (the keys in enumeration order) work as for the set. Also:

| Op | Meaning | Expected |
|---|---|---|
| `["set", k, v]` | `dict[k] = v` | - |
| `["add", k, v]` | `Add(k, v)`; scenarios never add a key that is present | - |
| `["try_add", k, v, bool]` | `TryAdd(k, v)` | its return value |
| `["get", k, found, v]` | `TryGetValue(k, out v)` | found, and v (0 when not found) |
| `["pairs", [[k, v], ...]]` | enumerate the dictionary | key-value pairs in enumeration order |

### `priority_queue.jsonl` - `PriorityQueue<int, double>` and `PriorityQueue<int, int>`

The first op is `["new", "double"]` or `["new", "int"]`. Elements are ints; priorities are
doubles or ints, drawn from a small set so ties are common.

| Op | Meaning | Expected |
|---|---|---|
| `["enqueue", e, p]` | `Enqueue(e, p)` | - |
| `["dequeue", e]` | `Dequeue()` | the element |
| `["peek", e]` | `Peek()` | the element |
| `["count", n]` | `Count` | the count |
| `["clear"]` | `Clear()` | - |
| `["remove", e, found, removed_e, p]` | `Remove(e, out removedElement, out priority)` with the default equality comparer | found, the removed element and its priority (0 when not found) |
| `["drain", [[e, p], ...]]` | dequeue everything with `TryDequeue` | each element and priority in order |

Double priorities are written with `"R"` round-trip formatting, so they read back exactly.

### `sort.jsonl` - .NET's introsort with ties

Each scenario sorts a list of `[key, id]` pairs with a comparison on `key` only, so equal
keys tie, and records the order of the ids. All four ways the planner sorts are covered,
since they share one algorithm, but each is recorded in case they ever diverge:

| Op | Meaning |
|---|---|
| `["list_sort", [[key, id], ...], [ids]]` | `List<T>.Sort(Comparison<T>)` |
| `["list_sort_range", [[key, id], ...], index, count, [ids]]` | `List<T>.Sort(index, count, IComparer<T>)` |
| `["array_sort", [[key, id], ...], [ids]]` | `Array.Sort(T[], Comparison<T>)` |
| `["span_sort", [[key, id], ...], [ids]]` | `MemoryExtensions.Sort(Span<T>, Comparison<T>)` |

Lengths run from 0 to about 300, with sorted, reversed, all-equal, few-distinct and random
key patterns, so the insertion-sort, heapsort and partition paths all run.

### `math.jsonl` - rounding and conversions

| Op | Meaning |
|---|---|
| `["round", x, r]` | `Math.Round(x, 0)`, which rounds half to even |
| `["round_int", x, n]` | `(int)Math.Round(x, 0)` |
| `["to_int", x, n]` | `(int)x` (truncation), for in-range x only |
| `["ceiling", x, r]` | `Math.Ceiling(x)` (used at `AddElectricPoles.cs:373`, `:667`, and Delaunator's hash size) |
| `["floor", x, r]` | `Math.Floor(x)` (used in Delaunator's `HashKey`) |

Each op name is its own scenario (`round`, `round_int`, `to_int`, `ceiling`, `floor`). `x` and `r` use `"R"` formatting, always with a `.` or an exponent.
