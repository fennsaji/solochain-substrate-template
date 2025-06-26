#!/usr/bin/env bash

# Run additional validator node script
set -e

bootnode=$1
spec=${2:-chainspecs/local/specRaw.json}
path=${3:-/tmp/solochain-nodes/node}
name=${4:-node}
target=${5:-target/release}
nodekey=${6:-0000000000000000000000000000000000000000000000000000000000000002}
port=${7:-30334}
rpcport=${8:-9945}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BINARY="${SCRIPT_DIR}/$target/solochain-template-node"

echo "*** Starting Validator Node: $name ***"

"$NODE_BINARY" \
--validator \
--base-path "$path" \
--chain "$spec" \
--bootnodes "$bootnode" \
--port "$port" \
--rpc-port "$rpcport" \
--rpc-external --rpc-cors all \
--rpc-methods=Unsafe \
--node-key "$nodekey" \
--name "$name" \
--pruning archive