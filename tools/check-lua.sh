#!/usr/bin/env bash
#
# Runs the same two Lua checks CI runs, against the same Lua version, using Docker.
#
# Why this exists
# ---------------
# The generated Lua targets Lua 5.2, because Factorio runs a modified 5.2. Homebrew
# stopped shipping 5.2 (it has 5.4 and newer), so a Mac checkout typically has a `luac`
# several minor versions too new. Parsing 5.2-targeted code with a 5.5 parser proves
# very little, so before this script the only real check was CI.
#
# nickblah/lua:5.2-alpine is Lua 5.2.4 - the exact version the README's Lua performance
# log was measured against, and what CI installs via apt.
#
# The two checks are NOT redundant
# --------------------------------
# Syntax alone is not enough, and the failure it misses is the one most likely to happen.
# LINQ transpiles cleanly and parses cleanly. It emits "local Linq = System.Linq.Enumerable",
# which is nil because Collections.Linq is not in the CoreSystem load list, so it only
# fails when the module is actually loaded:
#
#   ./FactorioTools/InitializeContext.lua:3: attempt to index field 'Linq'
#
# That is a runtime failure inside Factorio, not a build error. Running the planner is
# what catches it, and it takes well under a second.
#
# Requirements
# ------------
#   - Docker (OrbStack or Docker Desktop). The image is ~11MB and is pulled on first run.
#
# Usage
# -----
#   tools/check-lua.sh
#
# Regenerate the Lua first if you changed the core:
#   pwsh src/lua/Invoke-LuaBuild.ps1
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="nickblah/lua:5.2-alpine"

if ! docker info > /dev/null 2>&1; then
  echo "Docker is not responding. Start OrbStack or Docker Desktop and try again." >&2
  exit 1
fi

echo "Using $IMAGE ($(docker run --rm "$IMAGE" lua -v 2>&1 | head -1))"

# The generated file count is worth printing: a check that silently found zero files
# would pass and prove nothing.
FILE_COUNT="$(find "$REPO_ROOT/src/lua" -name '*.lua' | wc -l | tr -d ' ')"
if [[ "$FILE_COUNT" -eq 0 ]]; then
  echo "No .lua files found under src/lua. Has the transpile been run?" >&2
  exit 1
fi

echo
echo "1/2 Syntax-checking $FILE_COUNT generated files..."
docker run --rm -v "$REPO_ROOT:/repo" -w /repo "$IMAGE" \
  sh -c "find src/lua -name '*.lua' -print0 | xargs -0 -n1 luac -p"
echo "    All $FILE_COUNT files parse under Lua 5.2."

echo
echo "2/2 Running the transpiled planner (this is the step that catches LINQ)..."
docker run --rm -v "$REPO_ROOT:/repo" -w /repo/src/lua "$IMAGE" lua sample.lua

echo
echo "Both checks passed. This mirrors the transpile-lua CI job."
