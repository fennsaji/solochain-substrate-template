#!/bin/bash

# 3-Node Network with Proper Consensus Synchronization
# Based on working metamui pattern
# This script is for testing. Do not use it directly for production environment.

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
CHAINSPECS_DIR="${SCRIPT_DIR}/chainspecs"
STORE_PATH="/tmp/solochain-nodes"
TARGET="target/release"

# Fixed node configuration
NODEKEY="c12b6d18942f5ee8528c8e2baf4e147b5c5c18710926ea492d09cbd9f6c9f82a"
PEERID="12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2"
BOOTNODE="/ip4/127.0.0.1/tcp/30333/p2p/$PEERID"

cleanup() {
    print_warning "Stopping all nodes..."
    
    # Stop PM2 processes if available
    if command -v pm2 >/dev/null 2>&1; then
        pm2 delete all 2>/dev/null || true
    fi
    
    # Kill any remaining processes
    pkill -f "solochain-template-node" 2>/dev/null || true
    
    print_success "All nodes stopped"
}

trap cleanup EXIT INT TERM

# Check dependencies
if [ ! -f "$NODE_BINARY" ]; then
    print_error "Node binary not found. Run: cargo build --release"
    exit 1
fi

if ! command -v pm2 >/dev/null 2>&1; then
    print_error "PM2 not found. Please install: npm install -g pm2"
    exit 1
fi

print_status "🚀 Starting 3-Node Network with Proper Consensus"
print_status "================================================"

# Clear cached data
print_status "Cleaning previous data..."
rm -rf "$STORE_PATH/node1" "$STORE_PATH/node2" "$STORE_PATH/node3"
rm -rf "$CHAINSPECS_DIR"
mkdir -p "$CHAINSPECS_DIR/local" "$STORE_PATH"

# Delete all previous chains
pm2 delete all 2>/dev/null || true
sleep 2

print_status "Building chain specification..."

# Build spec
"$NODE_BINARY" build-spec --disable-default-bootnode --chain local > "$CHAINSPECS_DIR/local/spec.json"
"$NODE_BINARY" build-spec --disable-default-bootnode --raw --chain="$CHAINSPECS_DIR/local/spec.json" > "$CHAINSPECS_DIR/local/specRaw.json"

print_success "✓ Chain specification built"
sleep 2

print_status "Starting nodes..."

# Create PM2 ecosystem file for better management
cat > "$SCRIPT_DIR/ecosystem.config.js" << EOF
module.exports = {
  apps: [
    {
      name: 'node-1',
      script: '$NODE_BINARY',
      args: [
        '--validator',
        '--base-path', '$STORE_PATH/node1',
        '--chain', '$CHAINSPECS_DIR/local/specRaw.json',
        '--port', '30333',
        '--rpc-port', '9944',
        '--node-key', '$NODEKEY',
        '--rpc-external',
        '--rpc-cors', 'all',
        '--rpc-methods=Unsafe',
        '--name', 'node-1',
        '--pruning', 'archive'
      ]
    },
    {
      name: 'node-2',
      script: '$NODE_BINARY',
      args: [
        '--validator',
        '--base-path', '$STORE_PATH/node2',
        '--chain', '$CHAINSPECS_DIR/local/specRaw.json',
        '--bootnodes', '$BOOTNODE',
        '--port', '30334',
        '--rpc-port', '9945',
        '--rpc-external',
        '--rpc-cors', 'all',
        '--rpc-methods=Unsafe',
        '--node-key', '0000000000000000000000000000000000000000000000000000000000000002',
        '--name', 'node-2',
        '--pruning', 'archive'
      ]
    },
    {
      name: 'node-3',
      script: '$NODE_BINARY',
      args: [
        '--validator',
        '--base-path', '$STORE_PATH/node3',
        '--chain', '$CHAINSPECS_DIR/local/specRaw.json',
        '--bootnodes', '$BOOTNODE',
        '--port', '30335',
        '--rpc-port', '9946',
        '--rpc-external',
        '--rpc-cors', 'all',
        '--rpc-methods=Unsafe',
        '--node-key', '0000000000000000000000000000000000000000000000000000000000000003',
        '--name', 'node-3',
        '--pruning', 'archive'
      ]
    }
  ]
};
EOF

# Start nodes sequentially
print_status "Starting Node 1 (bootnode)..."
pm2 start "$SCRIPT_DIR/ecosystem.config.js" --only node-1
sleep 5

print_status "Starting Node 2..."
pm2 start "$SCRIPT_DIR/ecosystem.config.js" --only node-2
sleep 3

print_status "Starting Node 3..."
pm2 start "$SCRIPT_DIR/ecosystem.config.js" --only node-3
sleep 3

print_success "✓ All nodes started"

# Insert keys for each node
print_status "Inserting validator keys..."

insert_keys() {
    local seed=$1
    local path=$2
    local node_name=$3
    
    print_status "Inserting keys for $node_name ($seed)..."
    
    # Insert MICC key
    "$NODE_BINARY" key insert \
        --base-path "$path" \
        --chain "$CHAINSPECS_DIR/local/specRaw.json" \
        --scheme Sr25519 \
        --suri "$seed" \
        --key-type micc
    
    # Insert GRANDPA key
    "$NODE_BINARY" key insert \
        --base-path "$path" \
        --chain "$CHAINSPECS_DIR/local/specRaw.json" \
        --scheme Ed25519 \
        --suri "$seed" \
        --key-type gran
    
    print_success "✓ Keys inserted for $node_name"
}

insert_keys "//Alice" "$STORE_PATH/node1" "Node 1"
sleep 2

insert_keys "//Bob" "$STORE_PATH/node2" "Node 2"
sleep 2

insert_keys "//Charlie" "$STORE_PATH/node3" "Node 3"
sleep 2

# Restart all nodes to load keys
print_status "Restarting nodes to load keys..."
pm2 restart all
sleep 10

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

# Wait for nodes to fully initialize
sleep 5

test_node "Node 1 (Alice)" 9944
test_node "Node 2 (Bob)" 9945  
test_node "Node 3 (Charlie)" 9946

# Show network info
print_status ""
print_success "🎉 3-Node Network Started Successfully!"
print_status ""
print_status "📊 Node Information:"
print_status "==================="
print_status "Node 1 (Alice):   http://localhost:9944"
print_status "Node 2 (Bob):     http://localhost:9945"
print_status "Node 3 (Charlie): http://localhost:9946"
print_status ""
print_status "🌐 Connect via Polkadot-JS Apps:"
print_status "Node 1: https://polkadot.js.org/apps/?rpc=ws://localhost:9944"
print_status "Node 2: https://polkadot.js.org/apps/?rpc=ws://localhost:9945"
print_status "Node 3: https://polkadot.js.org/apps/?rpc=ws://localhost:9946"
print_status ""
print_status "📋 Features:"
print_status "• 3 validator nodes with proper consensus"
print_status "• Fee-free transactions on all nodes"
print_status "• MICC consensus with synchronized block production"
print_status "• Shared chain state across all nodes"
print_status "• Alice, Bob, Charlie as validators"
print_status ""
print_status "📋 PM2 Management:"
print_status "pm2 status        - Check node status"
print_status "pm2 logs          - View all logs"
print_status "pm2 logs node-1   - View specific node logs"
print_status "pm2 restart all   - Restart all nodes"
print_status "pm2 delete all    - Stop and remove all nodes"
print_status ""
print_status "📋 Data Locations:"
print_status "Nodes: $STORE_PATH/"
print_status "Chain Spec: $CHAINSPECS_DIR/local/"
print_status "PM2 Config: $SCRIPT_DIR/ecosystem.config.js"
print_status ""
print_warning "Use 'pm2 delete all' to stop all nodes"
print_warning "Or press Ctrl+C to stop this monitoring script"

# Monitor loop
while true; do
    sleep 30
    
    # Check PM2 status
    if ! pm2 status >/dev/null 2>&1; then
        print_error "PM2 process management failed"
        exit 1
    fi
    
    # Show status
    print_status "All 3 nodes running normally (PM2 managed)"
done