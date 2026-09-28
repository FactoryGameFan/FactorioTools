# A Linux container carrying every toolchain this repo's dev loop needs: the
# pinned .NET SDK plus wasm-tools, Node, PowerShell, Lua 5.2 and python3.
# CLAUDE.md's "Devcontainer" section says what it covers and what still needs
# the host (benchmarks and the Factorio oracle capture).
#
# Pinned to the exact SDK in global.json, by digest. That is the whole point of
# the container: `rollForward: latestMajor` only rolls FORWARD, so a host whose
# package manager trails the pin cannot build at all - measured 2026-09-21, when
# global.json moved to 10.0.401 and Homebrew still shipped 10.0.400.
#
# WHEN global.json MOVES, THIS LINE MUST MOVE WITH IT, and nothing enforces that.
# The Devcontainer CI job builds this image and asserts `dotnet --version`
# resolves inside the repo, which is what turns a forgotten bump into a red
# check rather than a container that silently disagrees with the repo.
# Re-pin with:
#   docker buildx imagetools inspect mcr.microsoft.com/dotnet/sdk:<version>
FROM mcr.microsoft.com/dotnet/sdk:10.0.401@sha256:2fa828c68761b1b8c23d7662dc134421b9d3b59fe1425fdbc80804e390cdb24d

# The base image is Ubuntu 24.04 (noble) and already has a `ubuntu` user at uid
# 1000, so there is no user to create - only sudo to add, for the postCreate
# chown of the named volumes.
#
# lua5.2 is the package CI installs, and on noble it is 5.2.4 - the exact
# version the README's Lua performance log was measured against, and what
# nickblah/lua:5.2-alpine provides in tools/check-lua.sh. It ships the binaries
# as `lua5.2` and `luac5.2`, which is what the transpile-lua CI job calls them;
# tools/check-lua.sh prefers them over Docker when they are on PATH.
#
# python3 is only for trim-factorio-oracle.py. The capture itself cannot run in
# here - it needs a real Factorio install - but the trim half is worth having.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        lua5.2 \
        python3 \
        sudo \
        xz-utils \
    && rm -rf /var/lib/apt/lists/* \
    && echo 'ubuntu ALL=(root) NOPASSWD:ALL' > /etc/sudoers.d/ubuntu \
    && chmod 0440 /etc/sudoers.d/ubuntu

# Node for src/vue. Pinned exactly and checksummed, rather than tracking a
# floating major the way ci.yml's `node-version: 24` does. The container is the
# place to be reproducible; CI tracking the major is a separate, deliberate
# choice, and the two are allowed to differ. v24.21.0 is the current 24 LTS
# (Krypton) and what this repo's contributors have on the host today.
#
# Both checksums are here because this image has to build on arm64 (a Mac) and
# amd64 (CI, and the Windows PC's WSL). `dpkg --print-architecture` picks.
ARG NODE_VERSION=v24.21.0
ARG NODE_SHA256_ARM64=6ad1325edbdb5649c379b75a237147a666c95d4f9ae8d340fef2d1575d289ad2
ARG NODE_SHA256_AMD64=fd8e59d5a511510f6a298afb548f18c7d2b1be404d8b4a27d94fbe49f56cb2d6
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
        arm64) node_arch=arm64; node_sha="${NODE_SHA256_ARM64}" ;; \
        amd64) node_arch=x64;   node_sha="${NODE_SHA256_AMD64}" ;; \
        *) echo "unsupported architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac; \
    curl -fsSLo /tmp/node.tar.xz \
        "https://nodejs.org/dist/${NODE_VERSION}/node-${NODE_VERSION}-linux-${node_arch}.tar.xz"; \
    echo "${node_sha}  /tmp/node.tar.xz" | sha256sum -c -; \
    tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 --no-same-owner \
        --exclude=CHANGELOG.md --exclude=LICENSE --exclude=README.md; \
    rm /tmp/node.tar.xz; \
    node --version; \
    npm --version

# PowerShell, because src/lua/Invoke-LuaBuild.ps1 is the only way to regenerate
# src/lua and the transpile-lua CI job runs it with `pwsh -File`.
#
# Installed as a dotnet tool rather than from Microsoft's apt repo: the tool
# feed carries arm64 and amd64 under one command, so this stays one code path
# for both architectures. The script itself sets DOTNET_ROLL_FORWARD=Major so
# the net7.0 CSharp.lua launcher runs on this .NET 10 runtime - no .NET 7
# runtime is needed here, despite what the launcher's target framework implies.
ARG POWERSHELL_VERSION=7.6.5
ENV DOTNET_TOOL_PATH=/usr/local/dotnet-tools
ENV PATH=${DOTNET_TOOL_PATH}:${PATH}
RUN dotnet tool install PowerShell \
        --version "${POWERSHELL_VERSION}" \
        --tool-path "${DOTNET_TOOL_PATH}" \
    && pwsh --version

# Needed by BrowserWasm and BlazorWebApp, so a full-solution `dotnet build`
# works in here. Installed at image build so it is not a surprise later:
# docker-build.sh has to tell you to prepend `dotnet workload restore`, and the
# point of this image is that you do not have to.
RUN dotnet workload install wasm-tools --skip-sign-check \
    && dotnet workload list

ENV DOTNET_CLI_TELEMETRY_OPTOUT=1 \
    DOTNET_NOLOGO=1

USER ubuntu
