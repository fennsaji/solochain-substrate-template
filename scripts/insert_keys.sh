#!/usr/bin/env bash

# Insert validator keys script
# Pass base seed, spec path and base path respectively as arguments
set -e

seed=$1
spec=${2:-chainspecs/local/specRaw.json}
path=${3:-/tmp/solochain-nodes/node}
target=${4:-target/release}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BINARY="${SCRIPT_DIR}/$target/solochain-template-node"

echo "*** Inserting keys ***"
echo ""
echo "seed: $seed"
echo "spec: $spec"
echo "path: $path"
echo ""

# Insert MICC consensus key
"$NODE_BINARY" key insert \
  --base-path "$path" \
  --chain "$spec" \
  --scheme Sr25519 \
  --suri "$seed"  \
  --key-type micc

# Insert GRANDPA finality key
"$NODE_BINARY" key insert \
  --base-path "$path" \
  --chain "$spec" \
  --scheme Ed25519 \
  --suri "$seed"  \
  --key-type gran

echo "Added micc and gran keys for $seed"