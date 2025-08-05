#!/bin/bash

# Stop all blockchain nodes script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="${SCRIPT_DIR}/blockchain-data"
NODES_DATA_DIR="${SCRIPT_DIR}/nodes-data"
SIMPLE_NODES_DIR="${SCRIPT_DIR}/simple-nodes"

print_status "Stopping all blockchain nodes..."

# Kill all node processes
pkill -f "solochain-template-node" || true

# Clean up PID files
for i in 1 2 3; do
    pid_file="${DATA_DIR}/node${i}/node.pid"
    if [ -f "$pid_file" ]; then
        rm "$pid_file"
        print_status "Cleaned up PID file for node $i"
    fi
done

# Optional: Clean up data directories
if [ "$1" = "--clean" ]; then
    print_warning "Cleaning up all node data..."
    rm -rf "$DATA_DIR" "$NODES_DATA_DIR" "$SIMPLE_NODES_DIR"
    print_success "All node data cleaned"
else
    print_status "Node data preserved. Use --clean to remove all data."
fi

print_success "All nodes stopped successfully"