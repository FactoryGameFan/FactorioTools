# A Linux container carrying every toolchain this repo's dev loop needs: the
# pinned Rust toolchain, Node and python3.
# CLAUDE.md's "Devcontainer" section says what it covers and what still needs
# the host (the Factorio oracle capture).
#
# Plain Ubuntu 26.04 (resolute), pinned by digest so every rebuild starts from
# the same bytes. It was the .NET SDK image (itself noble, 24.04) until the C#
# was removed, then plain 24.04; the apt packages below installed unchanged on
# both moves.
# Re-pin with:
#   docker buildx imagetools inspect ubuntu:26.04
FROM ubuntu:26.04@sha256:6a06111c3b0a8600feadb270aec771dc239bf407b25625f93f44ab00e11f4d3c

# The base image already has a `ubuntu` user at uid 1000, so there is no user
# to create - only sudo to add, for the postCreate chown of the named volumes.
#
# git is for vite.config.ts, which stamps the build with `git describe`.
#
# python3 is for trim-factorio-oracle.py, and for the Devcontainer CI job's
# toolchain check, which reads rust/rust-toolchain.toml with tomllib. The
# capture itself cannot run in here - it needs a real Factorio install - but
# the trim half is worth having.
#
# gcc and libc6-dev are for Rust: cargo links native code (build scripts, proc
# macros, the test binaries) with `cc`. Nothing in rust/Cargo.lock compiles C,
# so the rest of build-essential (g++, make) is not needed. The wasm32 build
# links with rust-lld, which ships with the toolchain.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        gcc \
        git \
        libc6-dev \
        python3 \
        sudo \
        xz-utils \
    && rm -rf /var/lib/apt/lists/* \
    && echo 'ubuntu ALL=(root) NOPASSWD:ALL' > /etc/sudoers.d/ubuntu \
    && chmod 0440 /etc/sudoers.d/ubuntu

# Node for src/vue. Pinned exactly and checksummed, rather than tracking a
# floating major the way ci.yml's `node-version: 26` does. The container is the
# place to be reproducible; CI tracking the major is a separate, deliberate
# choice, and the two are allowed to differ. v26.13.0 is the first 26 LTS
# (Lithium), released 2026-10-28.
#
# Both checksums are here because this image has to build on arm64 (a Mac) and
# amd64 (CI, and the Windows PC's WSL). `dpkg --print-architecture` picks. They
# come from https://nodejs.org/dist/v26.13.0/SHASUMS256.txt.
ARG NODE_VERSION=v26.13.0
ARG NODE_SHA256_ARM64=406fe3fbde1c054adff8b297c16fdb5af6659db327c1ac8acd46b56ef1af9a92
ARG NODE_SHA256_AMD64=d28a96af6c38596f6d9d97c0ee531c13d238d757742387d1928de822d959e997
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

USER ubuntu

# Rust, for rust/ and for `npm run build-wasm` in src/vue, which builds the
# planner the site loads. Installed as ubuntu into its home, so cargo can write
# its caches and rustup can add a toolchain without sudo.
#
# rustup-init is pinned and checksummed the same way Node is above. The
# checksums are the .sha256 files published beside each binary, and match a
# download of both. Self-update is off so the pin holds: otherwise
# `rustup toolchain install` checks for a newer rustup and installs it.
#
# rust/rust-toolchain.toml is the authority on the compiler: rustup reads it
# inside rust/, and installs whatever it names on first use. RUST_VERSION only
# decides what is ALREADY in the image, so that creating the container, and
# every run of the CI job, does not download a toolchain. rustfmt, clippy and
# the wasm32 target are what the toolchain file asks for; rust-src is extra,
# for rust-analyzer, which cannot index std without it.
#
# WHEN rust-toolchain.toml MOVES, RUST_VERSION SHOULD MOVE WITH IT. A stale one
# still works - rustup just downloads the new pin on first use - which is
# exactly why nobody would notice. The Devcontainer CI job checks the two agree.
ARG RUSTUP_VERSION=1.29.1
ARG RUSTUP_SHA256_ARM64=15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433
ARG RUSTUP_SHA256_AMD64=dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71
ENV RUST_VERSION=1.99.1 \
    CARGO_HOME=/home/ubuntu/.cargo \
    RUSTUP_HOME=/home/ubuntu/.rustup
ENV PATH=${CARGO_HOME}/bin:${PATH}
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
        arm64) rust_triple=aarch64-unknown-linux-gnu; rustup_sha="${RUSTUP_SHA256_ARM64}" ;; \
        amd64) rust_triple=x86_64-unknown-linux-gnu;  rustup_sha="${RUSTUP_SHA256_AMD64}" ;; \
        *) echo "unsupported architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac; \
    curl -fsSLo /tmp/rustup-init \
        "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${rust_triple}/rustup-init"; \
    echo "${rustup_sha}  /tmp/rustup-init" | sha256sum -c -; \
    chmod +x /tmp/rustup-init; \
    /tmp/rustup-init -y --no-modify-path \
        --profile minimal \
        --default-toolchain "${RUST_VERSION}" \
        --component rustfmt,clippy,rust-src \
        --target wasm32-unknown-unknown; \
    rm /tmp/rustup-init; \
    rustup set auto-self-update disable; \
    mkdir -p "${CARGO_HOME}/registry" "${CARGO_HOME}/git"; \
    rustup --version; \
    rustc --version; \
    cargo --version
