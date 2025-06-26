#!/usr/bin/env bash

# Build chain specification script
set -e

path=${1:-chainspecs/local}
chain=${2:-local}
target=${3:-target/release}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BINARY="${SCRIPT_DIR}/$target/solochain-template-node"

echo "*** Building Chain Specification ***"
echo "Path: $path"
echo "Chain: $chain"
echo "Target: $target"
echo ""

# Create directory
rm -rf "$path/"
mkdir -p "$path/"

# Build spec
"$NODE_BINARY" build-spec --disable-default-bootnode --chain "$chain" > "$path/spec.json"
"$NODE_BINARY" build-spec --disable-default-bootnode --raw --chain="$path/spec.json" > "$path/specRaw.json"

echo "Chain specification built successfully"