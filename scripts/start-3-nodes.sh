#!/bin/bash

# Simple 3-Node Network - Uses Standard Mode
# Avoids the event-driven consensus issues

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
DATA_DIR="${SCRIPT_DIR}/simple-nodes"

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

print_status "🚀 Starting Simple 3-Node Network"
print_status "================================="

# Start 3 separate development nodes on different ports
# This avoids the complex multi-node consensus issues

# Node 1 - Alice
print_status "Starting Node 1 (Alice)..."
"$NODE_BINARY" \
    --dev \
    --base-path "$DATA_DIR/node1" \
    --port 30333 \
    --rpc-port 9944 \
    --unsafe-rpc-external \
    --rpc-cors all \
    --rpc-methods unsafe \
    --name "Node1-Alice" \
    --no-telemetry \
    > "$DATA_DIR/node1.log" 2>&1 &

NODE1_PID=$!
print_success "✓ Node 1 started (PID: $NODE1_PID)"

sleep 8

# Node 2 - Bob  
print_status "Starting Node 2 (Bob)..."
"$NODE_BINARY" \
    --dev \
    --base-path "$DATA_DIR/node2" \
    --port 30334 \
    --rpc-port 9945 \
    --unsafe-rpc-external \
    --rpc-cors all \
    --rpc-methods unsafe \
    --name "Node2-Bob" \
    --no-telemetry \
    > "$DATA_DIR/node2.log" 2>&1 &

NODE2_PID=$!
print_success "✓ Node 2 started (PID: $NODE2_PID)"

sleep 5

# Node 3 - Charlie
print_status "Starting Node 3 (Charlie)..."
"$NODE_BINARY" \
    --dev \
    --base-path "$DATA_DIR/node3" \
    --port 30335 \
    --rpc-port 9946 \
    --unsafe-rpc-external \
    --rpc-cors all \
    --rpc-methods unsafe \
    --name "Node3-Charlie" \
    --no-telemetry \
    > "$DATA_DIR/node3.log" 2>&1 &

NODE3_PID=$!
print_success "✓ Node 3 started (PID: $NODE3_PID)"

# Wait for all nodes to initialize
print_status "Waiting for nodes to initialize... (15 seconds)"
sleep 15

# Test connectivity
print_status "🔍 Testing Node Health"
print_status "======================"

test_node() {
    local name=$1
    local port=$2
    local response=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' \
        "http://localhost:$port" 2>/dev/null || echo "")
    
    if echo "$response" | grep -q '"result"'; then
        print_success "✓ $name: Online and responding"
        return 0
    else
        print_error "✗ $name: Not responding"
        return 1
    fi
}

test_node "Node 1 (Alice)" 9944
test_node "Node 2 (Bob)" 9945  
test_node "Node 3 (Charlie)" 9946

# Show network info
print_status ""
print_success "🎉 3-Node Network Started Successfully!"
print_status ""
print_status "📊 Node Information:"
print_status "==================="
print_status "Node 1 (Alice):   http://localhost:9944 (PID: $NODE1_PID)"
print_status "Node 2 (Bob):     http://localhost:9945 (PID: $NODE2_PID)"
print_status "Node 3 (Charlie): http://localhost:9946 (PID: $NODE3_PID)"
print_status ""
print_status "🌐 Connect via Polkadot-JS Apps:"
print_status "Node 1: https://polkadot.js.org/apps/?rpc=ws://localhost:9944"
print_status "Node 2: https://polkadot.js.org/apps/?rpc=ws://localhost:9945"
print_status "Node 3: https://polkadot.js.org/apps/?rpc=ws://localhost:9946"
print_status ""
print_status "📋 Features:"
print_status "• Each node runs independently (dev mode)"
print_status "• Fee-free transactions on all nodes"
print_status "• MICC consensus with event-driven block production"
print_status "• Independent chain states (not synced between nodes)"
print_status ""
print_status "📋 Log Files:"
print_status "Node 1: $DATA_DIR/node1.log"
print_status "Node 2: $DATA_DIR/node2.log"
print_status "Node 3: $DATA_DIR/node3.log"
print_status ""
print_warning "Press Ctrl+C to stop all nodes"

# Monitor loop
while true; do
    sleep 30
    
    # Check if nodes are still running
    for pid in $NODE1_PID $NODE2_PID $NODE3_PID; do
        if ! kill -0 $pid 2>/dev/null; then
            print_error "A node has stopped unexpectedly (PID: $pid)"
            exit 1
        fi
    done
    
    # Show status
    print_status "All 3 nodes running normally"
done