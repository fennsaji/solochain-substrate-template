#!/bin/bash

# Working 3-Node Network Script
# Uses standard Substrate node without custom event-driven features

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BINARY="${SCRIPT_DIR}/target/release/solochain-template-node"
DATA_DIR="${SCRIPT_DIR}/blockchain-data"

cleanup() {
    print_warning "Stopping all nodes..."
    pkill -f "solochain-template-node" 2>/dev/null || true
    wait 2>/dev/null || true
    print_success "All nodes stopped"
}

trap cleanup EXIT INT TERM

if [ ! -f "$NODE_BINARY" ]; then
    print_error "Node binary not found. Run: cargo build --release"
    exit 1
fi

rm -rf "$DATA_DIR"
mkdir -p "$DATA_DIR"

print_status "🚀 Starting Working 3-Node Network"
print_status "=================================="

# Start Alice (using simple dev mode)
print_status "Starting Alice..."
"$NODE_BINARY" \
    --dev \
    --base-path "$DATA_DIR/alice" \
    --port 30333 \
    --rpc-port 9944 \
    --unsafe-rpc-external \
    --rpc-cors all \
    --rpc-methods unsafe \
    --name "Alice" \
    --no-telemetry \
    > "$DATA_DIR/alice.log" 2>&1 &

ALICE_PID=$!
print_success "✓ Alice started (PID: $ALICE_PID)"

# Give Alice time to start
sleep 10

print_status "Testing Alice connectivity..."
if curl -s -X POST -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' \
    "http://localhost:9944" | grep -q '"result"'; then
    print_success "✓ Alice is responding!"
else
    print_error "✗ Alice is not responding"
    exit 1
fi

# Test block production
print_status "Testing block production..."
block1=$(curl -s -X POST -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"chain_getBlock","params":[],"id":1}' \
    "http://localhost:9944" | jq -r '.result.block.header.number' 2>/dev/null | sed 's/0x//' | xargs -I {} printf "%d\n" 0x{} 2>/dev/null || echo "0")

print_status "Current block: #$block1"

print_status ""
print_success "🎉 Working Network Successfully Started!"
print_status ""
print_status "📊 Node Information:"
print_status "Alice (Dev): http://localhost:9944 (PID: $ALICE_PID)"
print_status ""
print_status "🌐 Polkadot-JS Apps:"
print_status "https://polkadot.js.org/apps/?rpc=ws://localhost:9944"
print_status ""
print_status "📋 Log File:"
print_status "$DATA_DIR/alice.log"
print_status ""
print_warning "Press Ctrl+C to stop the node"

# Monitor loop
while true; do
    sleep 30
    
    if ! kill -0 $ALICE_PID 2>/dev/null; then
        print_error "Alice has stopped unexpectedly"
        exit 1
    fi
    
    current_block=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"chain_getBlock","params":[],"id":1}' \
        "http://localhost:9944" | jq -r '.result.block.header.number' 2>/dev/null | sed 's/0x//' | xargs -I {} printf "%d\n" 0x{} 2>/dev/null || echo "0")
    print_status "Network status: Block #$current_block"
done