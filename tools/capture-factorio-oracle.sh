#!/usr/bin/env bash
#
# Captures an "oracle" of Factorio prototype facts that the oil field planner depends on,
# and writes it to a small committed JSON fixture.
#
# Why this exists
# ---------------
# The planner hardcodes entity names, item names, direction values and entity geometry.
# When Factorio changes any of those, nothing in this repo notices: plans keep generating,
# they are just wrong. Factorio 2.0 renamed "effectivity-module-N" to "efficiency-module-N"
# and moved directions from an 8-way to a 16-way encoding, and both went unnoticed here.
#
# So instead of trusting memory or the wiki, this pulls the facts out of the game itself.
# The game is the only authority on what the game accepts.
#
# What it is NOT
# --------------
# This is maintainer-only tooling. It is not part of the build and CI never runs it.
# CI reads the committed fixture, which is why the fixture is committed rather than
# generated on demand - CI machines have no Factorio install and never will.
#
# Requirements
# ------------
#   - A Factorio install (Steam or standalone). Only needed to re-capture, not to build.
#   - python3, for JSON trimming. Bash cannot do this legibly.
#
# Usage
# -----
#   tools/capture-factorio-oracle.sh
#   tools/capture-factorio-oracle.sh --check
#   tools/capture-factorio-oracle.sh --factorio /path/to/factorio.app --out some/file.json
#
# The installed binary is the authority on which version is captured. Steam updates it
# without asking, so it decides and everything else follows. This mirrors the convention
# in FactorioMapWebUI's scripts/sync-factorio-refs.sh.
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$REPO_ROOT/test/FactorioTools.Test/OilField/factorio-oracle.json"
OUT="$FIXTURE"
FACTORIO_APP=""
USER_DATA_DIR=""
CHECK_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --factorio) FACTORIO_APP="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --user-data-dir) USER_DATA_DIR="$2"; shift 2 ;;
    --check) CHECK_ONLY=1; shift ;;
    -h|--help) sed -n '2,36p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

# One scratch directory and one EXIT trap for the whole script. Registering a second
# trap on EXIT would silently replace the first and leak the earlier directory.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# --check captures to a scratch file and reports drift instead of rewriting the fixture,
# so "has the game moved past what we committed?" is answerable without a dirty tree.
if [[ "$CHECK_ONLY" -eq 1 ]]; then
  OUT="$WORK/factorio-oracle.json"
fi

# ---------------------------------------------------------------------------
# 1. Locate the game.
# ---------------------------------------------------------------------------
if [[ -z "$FACTORIO_APP" ]]; then
  for candidate in \
    "$HOME/Library/Application Support/Steam/steamapps/common/Factorio/factorio.app" \
    "/Applications/factorio.app" \
    "$HOME/.steam/steam/steamapps/common/Factorio" \
    "/opt/factorio"
  do
    if [[ -e "$candidate" ]]; then FACTORIO_APP="$candidate"; break; fi
  done
fi

if [[ -z "$FACTORIO_APP" || ! -e "$FACTORIO_APP" ]]; then
  echo "Could not find Factorio. Pass --factorio <path>." >&2
  exit 1
fi

# macOS ships it as an .app bundle; Linux as a plain directory.
if [[ -d "$FACTORIO_APP/Contents" ]]; then
  BIN="$FACTORIO_APP/Contents/MacOS/factorio"
  DATA_DIR="$FACTORIO_APP/Contents/data"
  DOC_DIR="$FACTORIO_APP/Contents/doc-html"
elif [[ -x "$FACTORIO_APP" ]]; then
  BIN="$FACTORIO_APP"
  DATA_DIR="$(dirname "$FACTORIO_APP")/../data"
  DOC_DIR="$(dirname "$FACTORIO_APP")/../doc-html"
else
  BIN="$FACTORIO_APP/bin/x64/factorio"
  DATA_DIR="$FACTORIO_APP/data"
  DOC_DIR="$FACTORIO_APP/doc-html"
fi

for required in "$BIN" "$DATA_DIR" "$DOC_DIR"; do
  if [[ ! -e "$required" ]]; then
    echo "Expected to find '$required' but it is missing. Is --factorio pointing at a real install?" >&2
    exit 1
  fi
done

VERSION="$("$BIN" --version | head -1 | sed -E 's/^Version: ([0-9.]+).*/\1/')"
echo "Factorio $VERSION at $FACTORIO_APP"

# ---------------------------------------------------------------------------
# 2. Dump data.raw with user mods disabled.
#
# This matters more than it looks. Running --dump-data with the default mod directory
# loads whatever the player happens to have installed, and mods freely rewrite prototypes.
# An oracle captured that way describes one person's modded game, not Factorio. Pointing
# --mod-directory at an empty directory leaves only core, base and the bundled DLC
# (elevated-rails, quality, recycler, space-age), which is what the planner targets.
# ---------------------------------------------------------------------------
mkdir -p "$WORK/mods"
printf '{"mods":[{"name":"base","enabled":true}]}\n' > "$WORK/mods/mod-list.json"

# Recorded before launching Factorio, so the dump file found below can be proven to be one
# this run actually wrote, rather than a stale one left over from an earlier capture.
DUMP_START="$(date +%s)"

echo "Dumping data.raw (mods disabled)..."
DUMP_LOG="$WORK/dump.log"
if ! "$BIN" --dump-data --mod-directory "$WORK/mods" > "$DUMP_LOG" 2>&1; then
  echo "factorio --dump-data failed:" >&2
  tail -30 "$DUMP_LOG" >&2
  exit 1
fi

# Report which mods actually loaded, so a contaminated capture is visible rather than silent.
LOADED_MODS="$(grep -oE 'Loading mod [A-Za-z0-9_-]+' "$DUMP_LOG" | sed 's/Loading mod //' | sort -u | tr '\n' ' ')"
echo "Loaded: $LOADED_MODS"

if [[ -z "$USER_DATA_DIR" ]]; then
  for candidate in \
    "$HOME/Library/Application Support/factorio" \
    "$HOME/.factorio" \
    "$FACTORIO_APP"
  do
    if [[ -f "$candidate/script-output/data-raw-dump.json" ]]; then USER_DATA_DIR="$candidate"; break; fi
  done
fi

DUMP="$USER_DATA_DIR/script-output/data-raw-dump.json"
if [[ ! -f "$DUMP" ]]; then
  echo "Could not find data-raw-dump.json. Pass --user-data-dir <path>." >&2
  exit 1
fi

# A dump whose modification time predates this run was not written by the --dump-data call
# above - it is left over from some earlier capture, possibly of a different Factorio version,
# sitting in a directory this run happened to search. Trimming it anyway would silently stamp
# stale prototype data with the version string of the game that just ran, which is exactly the
# silent pass this tool exists to prevent. BSD stat (macOS) and GNU stat (Linux) take the
# modification time differently, hence the fallback.
DUMP_MTIME="$(stat -f %m "$DUMP" 2>/dev/null || stat -c %Y "$DUMP")"
if [[ "$DUMP_MTIME" -lt "$DUMP_START" ]]; then
  echo "Found $DUMP, but its modification time predates this run." >&2
  echo "It looks like a stale dump left over from an earlier capture, not one this run wrote." >&2
  echo "Delete it and re-run, or pass --user-data-dir to point at the directory this run used." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# 3. Trim ~28MB of prototypes down to the handful of facts the planner relies on.
# ---------------------------------------------------------------------------
echo "Trimming to fixture..."
DUMP="$DUMP" DATA_DIR="$DATA_DIR" DOC_DIR="$DOC_DIR" \
VERSION="$VERSION" LOADED_MODS="$LOADED_MODS" OUT="$OUT" \
  python3 "$REPO_ROOT/tools/trim-factorio-oracle.py"

if [[ "$CHECK_ONLY" -eq 1 ]]; then
  if diff -u "$FIXTURE" "$OUT" > "$WORK/drift.diff" 2>&1; then
    echo "Up to date: the committed fixture matches Factorio $VERSION."
    exit 0
  fi
  echo
  echo "DRIFT: the committed fixture does not match Factorio $VERSION."
  echo "Re-run without --check to update it, then review what moved."
  echo
  cat "$WORK/drift.diff"
  exit 1
fi

echo "Wrote $OUT"
