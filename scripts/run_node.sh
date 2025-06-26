#!/usr/bin/env bash

# Run bootnode (Node 1) script
set -e

nodekey=$1
spec=${2:-chainspecs/local/specRaw.json}
path=${3:-/tmp/solochain-nodes/node1}
target=${4:-target/release}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BINARY="${SCRIPT_DIR}/$target/solochain-template-node"

echo "*** Starting Bootnode (Node 1) ***"

"$NODE_BINARY" \
--validator \
--base-path "$path" \
--chain "$spec" \
--port 30333 \
--rpc-port 9944 \
--node-key "$nodekey" \
--rpc-external \
--rpc-cors all \
--rpc-methods=Unsafe \
--name node-1 \
--pruning archive