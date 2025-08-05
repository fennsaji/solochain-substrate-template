#!/bin/bash

# Network Test Script
# Tests the 3-node blockchain network to ensure it's working correctly

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

# Configuration
NODE1_PORT=9944
NODE2_PORT=9945
NODE3_PORT=9946

TOTAL_TESTS=0
PASSED_TESTS=0

# Function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    print_status "Test $TOTAL_TESTS: $test_name"
    
    if eval "$test_command"; then
        print_success "✓ PASSED: $test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        print_error "✗ FAILED: $test_name"
        return 1
    fi
}

# Function to make RPC call
rpc_call() {
    local port=$1
    local method=$2
    local params=$3
    
    curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" \
        "http://localhost:$port" 2>/dev/null
}

# Test if node is responding
test_node_responding() {
    local port=$1
    local response=$(rpc_call "$port" "system_health" "[]")
    echo "$response" | grep -q '"result"'
}

# Test node health
test_node_health() {
    local port=$1
    local response=$(rpc_call "$port" "system_health" "[]")
    echo "$response" | grep -q '"isSyncing":false'
}

# Test node peers
test_node_peers() {
    local port=$1
    local min_peers=$2
    local response=$(rpc_call "$port" "system_health" "[]")
    local peers=$(echo "$response" | jq -r '.result.peers' 2>/dev/null || echo "0")
    [ "$peers" -ge "$min_peers" ]
}

# Test block production
test_block_production() {
    local port=$1
    
    # Get current block number
    local response1=$(rpc_call "$port" "chain_getBlock" "[]")
    local block1=$(echo "$response1" | jq -r '.result.block.header.number' 2>/dev/null || echo "0")
    
    # Wait a bit
    sleep 12
    
    # Get new block number
    local response2=$(rpc_call "$port" "chain_getBlock" "[]")
    local block2=$(echo "$response2" | jq -r '.result.block.header.number' 2>/dev/null || echo "0")
    
    # Convert hex to decimal if needed
    if [[ "$block1" =~ ^0x ]]; then
        block1=$((block1))
    fi
    if [[ "$block2" =~ ^0x ]]; then
        block2=$((block2))
    fi
    
    [ "$block2" -gt "$block1" ]
}

# Test runtime API
test_runtime_api() {
    local port=$1
    local response=$(rpc_call "$port" "state_getRuntimeVersion" "[]")
    echo "$response" | grep -q '"specName"'
}

# Test balance query (should work even without fees)
test_balance_query() {
    local port=$1
    # Query Alice's balance
    local alice_ss58="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
    local response=$(rpc_call "$port" "system_accountNextIndex" "[\"$alice_ss58\"]")
    echo "$response" | grep -q '"result"'
}

# Test metadata query
test_metadata() {
    local port=$1
    local response=$(rpc_call "$port" "state_getMetadata" "[]")
    echo "$response" | grep -q '"result"'
}

# Test chain information
test_chain_info() {
    local port=$1
    local response=$(rpc_call "$port" "system_chain" "[]")
    echo "$response" | grep -q '"result"'
}

# Main test execution
main() {
    print_status "Blockchain Network Test Suite"
    print_status "============================="
    
    # Test each node individually
    for i in 1 2 3; do
        local port_var="NODE${i}_PORT"
        local port=${!port_var}
        
        print_status ""
        print_status "Testing Node $i (Port $port)"
        print_status "------------------------"
        
        run_test "Node $i - Basic Connectivity" "test_node_responding $port"
        run_test "Node $i - Health Check" "test_node_health $port"
        run_test "Node $i - Runtime API" "test_runtime_api $port"
        run_test "Node $i - Chain Info" "test_chain_info $port"
        run_test "Node $i - Balance Query" "test_balance_query $port"
        run_test "Node $i - Metadata Query" "test_metadata $port"
    done
    
    # Network tests
    print_status ""
    print_status "Network Connectivity Tests"
    print_status "-------------------------"
    
    # Test peer connections (each node should have at least 1 peer)
    run_test "Node 1 - Peer Connectivity" "test_node_peers $NODE1_PORT 1"
    run_test "Node 2 - Peer Connectivity" "test_node_peers $NODE2_PORT 1"
    run_test "Node 3 - Peer Connectivity" "test_node_peers $NODE3_PORT 1"
    
    # Test block production
    print_status ""
    print_status "Consensus Tests"
    print_status "--------------"
    print_status "Testing block production (this may take ~12 seconds)..."
    
    run_test "Node 1 - Block Production" "test_block_production $NODE1_PORT"
    
    # Summary
    print_status ""
    print_status "Test Results Summary"
    print_status "==================="
    
    if [ "$PASSED_TESTS" -eq "$TOTAL_TESTS" ]; then
        print_success "All tests passed! ($PASSED_TESTS/$TOTAL_TESTS)"
        print_success "🎉 Your 3-node blockchain network is working correctly!"
        
        print_status ""
        print_status "Network Information:"
        print_status "Node 1: http://localhost:$NODE1_PORT"
        print_status "Node 2: http://localhost:$NODE2_PORT" 
        print_status "Node 3: http://localhost:$NODE3_PORT"
        print_status ""
        print_status "You can connect to any node using Polkadot-JS Apps:"
        print_status "https://polkadot.js.org/apps/?rpc=ws://localhost:$NODE1_PORT"
        
        exit 0
    else
        print_error "Some tests failed ($PASSED_TESTS/$TOTAL_TESTS passed)"
        print_error "❌ Network may not be functioning correctly"
        exit 1
    fi
}

# Check if jq is available
if ! command -v jq &> /dev/null; then
    print_warning "jq is not installed. Some tests may be less accurate."
    print_status "Install jq for better test results: brew install jq"
fi

# Run tests
main "$@"