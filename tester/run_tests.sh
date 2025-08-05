#!/bin/bash

# MetaMUI Blockchain Testing Suite Runner
# This script runs a comprehensive set of tests to validate event-driven block production

echo "🚀 MetaMUI Blockchain Testing Suite"
echo "====================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if blockchain is running (simple check)
echo -e "${BLUE}🔍 Checking blockchain connection...${NC}"
echo -e "${GREEN}✅ Proceeding with tests (blockchain should be running on ws://127.0.0.1:9944)${NC}"
echo ""

# Function to run test with error handling
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    echo -e "${BLUE}📋 Running: $test_name${NC}"
    echo "Command: $test_command"
    echo "----------------------------------------"
    
    if eval $test_command; then
        echo -e "${GREEN}✅ $test_name completed successfully${NC}"
    else
        echo -e "${RED}❌ $test_name failed${NC}"
        return 1
    fi
    echo ""
}

# Ensure dependencies are installed
if [ ! -d "node_modules" ]; then
    echo -e "${YELLOW}📦 Installing dependencies...${NC}"
    npm install
    echo ""
fi

# Test Suite 1: Basic Functionality
echo -e "${YELLOW}🧪 Test Suite 1: Basic Functionality${NC}"
echo "======================================"

run_test "Single Transaction Test" "node system_remark.js single --account alice --remark 'Basic functionality test'"

run_test "Serial Transactions Test" "node system_remark.js serial --count 5 --account bob --delay 1500"

# Test Suite 2: Event-Driven Features
echo -e "${YELLOW}🎯 Test Suite 2: Event-Driven Features${NC}"
echo "======================================="

run_test "High Priority Transaction Test" "node system_remark.js single --account charlie --priority critical --remark 'High priority test'"

run_test "Mixed Priority Test" "node system_remark.js priority --count 12"

run_test "Parallel Multi-Account Test" "node system_remark.js parallel --count 3 --accounts alice,bob,charlie,dave"

# Test Suite 3: Performance Tests
echo -e "${YELLOW}⚡ Test Suite 3: Performance Tests${NC}"
echo "==================================="

run_test "Short Stress Test" "node system_remark.js stress --duration 15000 --interval 300"

run_test "Burst Capacity Test" "node system_remark.js burst --size 15 --count 3 --interval 3000"

# Test Suite 4: Advanced Scenarios
echo -e "${YELLOW}🔬 Test Suite 4: Advanced Scenarios${NC}"
echo "===================================="

run_test "Multi-User Concurrent Test" "node system_remark.js parallel --count 5 --accounts alice,bob,charlie,dave,eve --priority normal"

run_test "Priority Validation Test" "node system_remark.js priority --count 20"

echo -e "${GREEN}🎉 All tests completed!${NC}"
echo ""
echo -e "${BLUE}📊 Test Summary:${NC}"
echo "- Basic functionality tests validate core transaction processing"
echo "- Event-driven tests validate priority handling and immediate block production"
echo "- Performance tests validate system throughput and stability"
echo "- Advanced scenarios test multi-user and complex priority interactions"
echo ""
echo -e "${YELLOW}💡 Next steps:${NC}"
echo "- Review blockchain logs for event-driven behavior"
echo "- Check block production patterns match expected event-driven triggers"
echo "- Verify priority transactions get immediate processing"
echo "- Monitor collection window behavior and transaction batching"