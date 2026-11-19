#!/bin/sh
# Build the normal, native Apple Silicon release. Run from a prepared source tree.
set -eu
cd "$(dirname "$0")/.."
[ "$(uname -m)" = arm64 ] || { echo 'Se requiere una Mac Apple Silicon con toolchain arm64.' >&2; exit 1; }
mkdir -p .development/temp-005
export TMPDIR="$PWD/.development/temp-005" CLANG_MODULE_CACHE_PATH="$PWD/.development/clang-cache"
if [ -x "$PWD/.development/cargo/bin/cargo" ]; then
  export CARGO_HOME="$PWD/.development/cargo" RUSTUP_HOME="$PWD/.development/rustup"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
export CARGO_NET_OFFLINE=true npm_config_offline=true npm_config_cache="$PWD/.development/npm-cache"
# Cargo and Rust may live outside the source folder on a new machine.
# Encoded flags also preserve paths containing spaces.
flag_separator=$(printf '\037')
project_remap="--remap-path-prefix=$PWD=."
cargo_remap="--remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=cargo"
rustup_remap="--remap-path-prefix=${RUSTUP_HOME:-$HOME/.rustup}=rustup"
export CARGO_ENCODED_RUSTFLAGS="$project_remap$flag_separator$cargo_remap$flag_separator$rustup_remap"
# package-release.py strips the staged executable with the macOS tool.
export CARGO_PROFILE_RELEASE_STRIP=none
npm run tauri -- build --bundles app -- --locked --offline
