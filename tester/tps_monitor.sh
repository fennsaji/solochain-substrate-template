#!/bin/bash

# TPS Monitoring Script - Integrates throughput testing with system monitoring
# This script runs throughput tests while monitoring system performance

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
METRICS_SCRIPT="$SCRIPT_DIR/stress_metrics.sh"
THROUGHPUT_SCRIPT="$SCRIPT_DIR/throughput_analyzer.js"
OUTPUT_DIR="./tps_monitoring_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# Default test parameters
DEFAULT_ENDPOINT="ws://127.0.0.1:9944"
DEFAULT_TEST_TYPE="baseline"
DEFAULT_DURATION=30000
DEFAULT_MAX_TPS=50

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_color() {
    echo -e "${1}${2}${NC}"
}

# Function to check dependencies
check_dependencies() {
    print_color $BLUE "🔍 Checking dependencies..."
    
    # Check if node is available
    if ! command -v node &> /dev/null; then
        print_color $RED "❌ Node.js is not installed or not in PATH"
        exit 1
    fi
    
    # Check if stress metrics script exists
    if [ ! -f "$METRICS_SCRIPT" ]; then
        print_color $RED "❌ Stress metrics script not found: $METRICS_SCRIPT"
        exit 1
    fi
    
    # Check if throughput analyzer exists
    if [ ! -f "$THROUGHPUT_SCRIPT" ]; then
        print_color $RED "❌ Throughput analyzer script not found: $THROUGHPUT_SCRIPT"
        exit 1
    fi
    
    # Make scripts executable
    chmod +x "$METRICS_SCRIPT" 2>/dev/null
    chmod +x "$THROUGHPUT_SCRIPT" 2>/dev/null
    
    print_color $GREEN "✅ All dependencies found"
}

# Function to create output directory
setup_output_dir() {
    mkdir -p "$OUTPUT_DIR" 2>/dev/null
    if [ $? -eq 0 ]; then
        print_color $GREEN "📁 Output directory created: $OUTPUT_DIR"
    else
        print_color $YELLOW "⚠️  Using current directory for output"
        OUTPUT_DIR="."
    fi
}

# Function to start system monitoring in background
start_monitoring() {
    local duration=$1
    local test_id=$2
    
    print_color $BLUE "📊 Starting system monitoring (${duration}s)..."
    
    # Calculate monitoring duration with buffer
    local monitor_duration=$((duration / 1000 + 10))
    
    # Start monitoring in background
    "$METRICS_SCRIPT" start $monitor_duration > "$OUTPUT_DIR/system_metrics_${test_id}_${TIMESTAMP}.log" 2>&1 &
    local monitor_pid=$!
    
    echo $monitor_pid
}

# Function to run throughput test with monitoring
run_monitored_test() {
    local test_type="$1"
    local endpoint="$2"
    local duration="$3"
    local additional_args="$4"
    local test_id="${test_type}_${TIMESTAMP}"
    
    print_color $YELLOW "🚀 Starting monitored TPS test: $test_type"
    print_color $BLUE "   Endpoint: $endpoint"
    print_color $BLUE "   Duration: ${duration}ms"
    print_color $BLUE "   Test ID: $test_id"
    
    # Start system monitoring
    local monitor_pid=$(start_monitoring $duration $test_id)
    
    # Prepare throughput test command
    local throughput_cmd="node \"$THROUGHPUT_SCRIPT\" --endpoint \"$endpoint\" --output \"$OUTPUT_DIR\" $test_type $additional_args"
    
    print_color $BLUE "🎯 Running: $throughput_cmd"
    
    # Run throughput test
    local start_time=$(date +%s)
    eval $throughput_cmd
    local test_exit_code=$?
    local end_time=$(date +%s)
    local actual_duration=$((end_time - start_time))
    
    # Wait for monitoring to complete (with timeout)
    local wait_count=0
    while kill -0 $monitor_pid 2>/dev/null && [ $wait_count -lt 30 ]; do
        sleep 1
        ((wait_count++))
    done
    
    # Force kill monitoring if still running
    if kill -0 $monitor_pid 2>/dev/null; then
        kill $monitor_pid 2>/dev/null
        print_color $YELLOW "⚠️  Monitoring process terminated"
    fi
    
    print_color $GREEN "✅ Test completed in ${actual_duration}s (exit code: $test_exit_code)"
    
    # Generate combined report
    generate_combined_report "$test_id" "$test_type" "$actual_duration" "$test_exit_code"
    
    return $test_exit_code
}

# Function to generate combined report
generate_combined_report() {
    local test_id="$1"
    local test_type="$2"
    local duration="$3"
    local exit_code="$4"
    
    local report_file="$OUTPUT_DIR/combined_report_${test_id}.txt"
    local metrics_file="$OUTPUT_DIR/system_metrics_${test_id}_${TIMESTAMP}.log"
    
    print_color $BLUE "📄 Generating combined report..."
    
    cat > "$report_file" << EOF
MetaMUI TPS Monitoring Report
============================
Generated: $(date)
Test ID: $test_id
Test Type: $test_type
Duration: ${duration}s
Exit Code: $exit_code

SYSTEM PERFORMANCE SUMMARY
==========================
EOF
    
    # Analyze system metrics if file exists
    if [ -f "$metrics_file" ]; then
        echo "" >> "$report_file"
        echo "System Metrics Analysis:" >> "$report_file"
        echo "------------------------" >> "$report_file"
        
        # Extract CPU usage statistics
        if grep -q "CPU Usage:" "$metrics_file"; then
            echo "CPU Usage:" >> "$report_file"
            grep "CPU Usage:" "$metrics_file" | awk -F': ' '{print $2}' | sed 's/%//' | awk '
            {
                sum += $1; count++; 
                if(NR==1 || $1 > max) max = $1; 
                if(NR==1 || $1 < min) min = $1
            } 
            END {
                if(count > 0) printf "  Average: %.2f%%, Min: %.2f%%, Max: %.2f%%\n", sum/count, min, max
            }' >> "$report_file"
        fi
        
        # Extract memory statistics
        if grep -q "Memory Free:" "$metrics_file"; then
            echo "Memory:" >> "$report_file"
            grep "Memory Free:" "$metrics_file" | awk -F': ' '{print $2}' | sed 's/%//' | awk '
            {
                sum += $1; count++; 
                if(NR==1 || $1 > max) max = $1; 
                if(NR==1 || $1 < min) min = $1
            } 
            END {
                if(count > 0) printf "  Average Free: %.2f%%, Min Free: %.2f%%, Max Free: %.2f%%\n", sum/count, min, max
            }' >> "$report_file"
        fi
        
        # Extract blockchain process info
        if grep -q "Active blockchain processes:" "$metrics_file"; then
            echo "Blockchain Processes:" >> "$report_file"
            grep "Active blockchain processes:" "$metrics_file" | awk -F': ' '{print $2}' | sort | uniq -c | awk '{print "  " $2 " processes active: " $1 " times"}' >> "$report_file"
        fi
        
        echo "" >> "$report_file"
        echo "Full system metrics available in: $metrics_file" >> "$report_file"
    else
        echo "System metrics file not found: $metrics_file" >> "$report_file"
    fi
    
    # Add throughput results if available
    echo "" >> "$report_file"
    echo "THROUGHPUT RESULTS" >> "$report_file"
    echo "==================" >> "$report_file"
    
    # Look for JSON result files
    local json_files=$(find "$OUTPUT_DIR" -name "throughput_analysis_*.json" -newer "$report_file" 2>/dev/null | head -1)
    if [ -n "$json_files" ]; then
        echo "Detailed throughput results available in: $json_files" >> "$report_file"
        
        # Extract summary if possible
        if command -v jq &> /dev/null; then
            echo "" >> "$report_file"
            echo "Quick Summary:" >> "$report_file"
            jq -r '.summary | to_entries[] | "  \(.key): \(.value)"' "$json_files" 2>/dev/null >> "$report_file"
        fi
    fi
    
    # Look for summary text files
    local summary_files=$(find "$OUTPUT_DIR" -name "throughput_summary_*.txt" -newer "$report_file" 2>/dev/null | head -1)
    if [ -n "$summary_files" ]; then
        echo "" >> "$report_file"
        echo "Summary from throughput analyzer:" >> "$report_file"
        echo "=================================" >> "$report_file"
        cat "$summary_files" >> "$report_file" 2>/dev/null
    fi
    
    print_color $GREEN "📊 Combined report saved: $report_file"
}

# Function to run comprehensive TPS analysis
run_comprehensive_analysis() {
    local endpoint="$1"
    local output_dir="$2"
    
    print_color $YELLOW "🔬 Starting Comprehensive TPS Analysis"
    print_color $BLUE "========================================"
    
    # Test configurations
    local tests=(
        "baseline --max-tps 30 --duration 20000:Quick Baseline"
        "concurrency --accounts 4 --tps 8 --duration 20000:Concurrency Test"
        "max-tps --initial 20 --max-attempts 5 --duration 10000:Max TPS"
        "priority --tps 15 --duration 15000:Priority Impact"
    )
    
    local total_tests=${#tests[@]}
    local current_test=1
    
    for test_config in "${tests[@]}"; do
        IFS=':' read -r test_args test_name <<< "$test_config"
        
        print_color $YELLOW "\n📋 Test $current_test/$total_tests: $test_name"
        print_color $BLUE "Arguments: $test_args"
        
        # Extract duration for monitoring
        local duration=$(echo "$test_args" | grep -o '\--duration [0-9]*' | awk '{print $2}')
        if [ -z "$duration" ]; then
            duration=20000  # Default
        fi
        
        # Run test with monitoring
        run_monitored_test "$test_args" "$endpoint" "$duration" ""
        
        # Brief pause between tests
        if [ $current_test -lt $total_tests ]; then
            print_color $BLUE "⏱️  Pausing 5 seconds before next test..."
            sleep 5
        fi
        
        ((current_test++))
    done
    
    print_color $GREEN "\n🎉 Comprehensive analysis complete!"
    print_color $BLUE "Results available in: $output_dir"
}

# Function to run quick test
run_quick_test() {
    local endpoint="$1"
    
    print_color $YELLOW "⚡ Running Quick TPS Test"
    run_monitored_test "baseline --max-tps 20 --duration 15000 --step 5" "$endpoint" "15000" ""
}

# Function to monitor existing test
monitor_existing_test() {
    local duration="$1"
    local test_name="$2"
    
    print_color $YELLOW "👀 Monitoring existing test: $test_name"
    
    local monitor_pid=$(start_monitoring $((duration * 1000)) "external_$test_name")
    
    print_color $BLUE "📊 Monitoring for ${duration} seconds..."
    print_color $BLUE "Press Ctrl+C to stop monitoring early"
    
    # Wait for specified duration or user interrupt
    sleep $duration
    
    # Stop monitoring
    if kill -0 $monitor_pid 2>/dev/null; then
        kill $monitor_pid 2>/dev/null
    fi
    
    print_color $GREEN "✅ Monitoring complete"
    generate_combined_report "external_$test_name" "monitoring" "$duration" "0"
}

# Function to show usage
show_usage() {
    cat << EOF
TPS Monitoring Script - Blockchain Throughput Testing with System Monitoring

Usage:
    $0 <command> [options]

Commands:
    quick [endpoint]                    - Run quick TPS test (default endpoint: $DEFAULT_ENDPOINT)
    baseline [endpoint] [max_tps]       - Run baseline TPS test
    concurrency [endpoint] [accounts]   - Run concurrency test
    max-tps [endpoint]                  - Find maximum sustainable TPS
    priority [endpoint]                 - Test priority impact
    ramp [endpoint]                     - Run load ramp test
    comprehensive [endpoint]            - Run full analysis suite
    monitor <duration> [test_name]      - Monitor existing test for duration (seconds)
    
Options:
    -h, --help                         - Show this help
    -o, --output <dir>                 - Output directory (default: $OUTPUT_DIR)
    -e, --endpoint <url>               - Blockchain endpoint (default: $DEFAULT_ENDPOINT)

Examples:
    $0 quick                                    # Quick test on localhost
    $0 baseline ws://127.0.0.1:9944 30        # Baseline test up to 30 TPS
    $0 comprehensive                           # Full analysis suite
    $0 monitor 60 stress_test                  # Monitor external test for 60 seconds
    $0 concurrency ws://127.0.0.1:9944 6      # Test with 6 concurrent accounts

Output:
    - System metrics: $OUTPUT_DIR/system_metrics_*.log
    - Throughput results: $OUTPUT_DIR/throughput_*.json
    - Combined reports: $OUTPUT_DIR/combined_report_*.txt

Dependencies:
    - Node.js (for throughput testing)
    - stress_metrics.sh (for system monitoring)
    - throughput_analyzer.js (for TPS testing)
EOF
}

# Function to parse arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_usage
                exit 0
                ;;
            -o|--output)
                OUTPUT_DIR="$2"
                shift 2
                ;;
            -e|--endpoint)
                DEFAULT_ENDPOINT="$2"
                shift 2
                ;;
            *)
                break
                ;;
        esac
    done
}

# Main execution
main() {
    # Parse global arguments first
    parse_args "$@"
    
    # Get remaining arguments
    local remaining_args=()
    while [[ $# -gt 0 ]]; do
        case $1 in
            -o|--output|-e|--endpoint)
                shift 2  # Skip these as they were already processed
                ;;
            *)
                remaining_args+=("$1")
                shift
                ;;
        esac
    done
    
    # Restore remaining arguments
    set -- "${remaining_args[@]}"
    
    local command="${1:-help}"
    
    # Setup
    check_dependencies
    setup_output_dir
    
    # Execute command
    case "$command" in
        "quick")
            local endpoint="${2:-$DEFAULT_ENDPOINT}"
            run_quick_test "$endpoint"
            ;;
        "baseline")
            local endpoint="${2:-$DEFAULT_ENDPOINT}"
            local max_tps="${3:-$DEFAULT_MAX_TPS}"
            run_monitored_test "baseline --max-tps $max_tps --duration $DEFAULT_DURATION" "$endpoint" "$DEFAULT_DURATION" ""
            ;;
        "concurrency")
            local endpoint="${2:-$DEFAULT_ENDPOINT}"
            local accounts="${3:-6}"
            run_monitored_test "concurrency --accounts $accounts --tps 10 --duration $DEFAULT_DURATION" "$endpoint" "$DEFAULT_DURATION" ""
            ;;
        "max-tps")
            local endpoint="${2:-$DEFAULT_ENDPOINT}"
            run_monitored_test "max-tps --initial 30 --duration 15000" "$endpoint" "90000" ""  # Longer duration for max-tps
            ;;
        "priority")
            local endpoint="${2:-$DEFAULT_ENDPOINT}"
            run_monitored_test "priority --tps 20 --duration $DEFAULT_DURATION" "$endpoint" "$DEFAULT_DURATION" ""
            ;;
        "ramp")
            local endpoint="${2:-$DEFAULT_ENDPOINT}"
            run_monitored_test "ramp --start-tps 5 --end-tps 50 --duration 60000" "$endpoint" "60000" ""
            ;;
        "comprehensive")
            local endpoint="${2:-$DEFAULT_ENDPOINT}"
            run_comprehensive_analysis "$endpoint" "$OUTPUT_DIR"
            ;;
        "monitor")
            local duration="${2:-60}"
            local test_name="${3:-external_test}"
            monitor_existing_test "$duration" "$test_name"
            ;;
        "help"|*)
            show_usage
            ;;
    esac
}

# Run main function with all arguments
main "$@"