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
#   tools/capture-factorio-oracle.sh --factorio /path/to/factorio.app --out some/file.json
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$REPO_ROOT/test/FactorioTools.Test/OilField/factorio-oracle.json"
FACTORIO_APP=""
USER_DATA_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --factorio) FACTORIO_APP="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --user-data-dir) USER_DATA_DIR="$2"; shift 2 ;;
    -h|--help) sed -n '2,32p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

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
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/mods"
printf '{"mods":[{"name":"base","enabled":true}]}\n' > "$WORK/mods/mod-list.json"

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

# ---------------------------------------------------------------------------
# 3. Trim ~28MB of prototypes down to the handful of facts the planner relies on.
# ---------------------------------------------------------------------------
echo "Trimming to fixture..."
DUMP="$DUMP" DATA_DIR="$DATA_DIR" DOC_DIR="$DOC_DIR" \
VERSION="$VERSION" LOADED_MODS="$LOADED_MODS" OUT="$OUT" \
  python3 "$REPO_ROOT/tools/trim-factorio-oracle.py"

echo "Wrote $OUT"
