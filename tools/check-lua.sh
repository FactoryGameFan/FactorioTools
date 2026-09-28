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
# Either of these, and the script picks on its own:
#   - A local Lua 5.2 on PATH as `lua5.2` / `luac5.2`. That is the apt package name
#     CI installs, and what the devcontainer carries, so inside the container this
#     is the path taken and no Docker is needed - which matters, because there is
#     no Docker daemon in there to fall back to.
#   - Docker (OrbStack or Docker Desktop). The image is ~11MB and is pulled on
#     first run. This is the path on a Mac host, where Homebrew has no 5.2.
#
# Only the binaries named `lua5.2`/`luac5.2` are accepted, deliberately. A bare
# `lua` is whatever the host happens to have - typically 5.4 or newer on a Mac -
# and parsing 5.2-targeted code with a newer parser proves very little, which is
# the whole reason this script exists. Version-suffixed names are unambiguous.
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

if command -v lua5.2 > /dev/null 2>&1 && command -v luac5.2 > /dev/null 2>&1; then
  USE_DOCKER=false
  echo "Using local Lua 5.2 ($(lua5.2 -v 2>&1 | head -1))"
elif docker info > /dev/null 2>&1; then
  USE_DOCKER=true
  echo "Using $IMAGE ($(docker run --rm "$IMAGE" lua -v 2>&1 | head -1))"
else
  echo "Need either lua5.2 + luac5.2 on PATH, or a running Docker (OrbStack or" >&2
  echo "Docker Desktop). Found neither." >&2
  exit 1
fi

# The generated file count is worth printing: a check that silently found zero files
# would pass and prove nothing.
FILE_COUNT="$(find "$REPO_ROOT/src/lua" -name '*.lua' | wc -l | tr -d ' ')"
if [[ "$FILE_COUNT" -eq 0 ]]; then
  echo "No .lua files found under src/lua. Has the transpile been run?" >&2
  exit 1
fi

echo
echo "1/2 Syntax-checking $FILE_COUNT generated files..."
if [[ "$USE_DOCKER" == true ]]; then
  docker run --rm -v "$REPO_ROOT:/repo" -w /repo "$IMAGE" \
    sh -c "find src/lua -name '*.lua' -print0 | xargs -0 -n1 luac -p"
else
  (cd "$REPO_ROOT" && find src/lua -name '*.lua' -print0 | xargs -0 -n1 luac5.2 -p)
fi
echo "    All $FILE_COUNT files parse under Lua 5.2."

echo
echo "2/2 Running the transpiled planner (this is the step that catches LINQ)..."
if [[ "$USE_DOCKER" == true ]]; then
  docker run --rm -v "$REPO_ROOT:/repo" -w /repo/src/lua "$IMAGE" lua sample.lua
else
  (cd "$REPO_ROOT/src/lua" && lua5.2 sample.lua)
fi

echo
echo "Both checks passed. This mirrors the transpile-lua CI job."
