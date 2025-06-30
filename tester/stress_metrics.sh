#!/bin/bash

# Blockchain Stress Test Metrics Collection Script
# This script monitors system performance during blockchain stress tests

# Configuration
METRICS_DIR="./stress_metrics"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
LOG_FILE="$METRICS_DIR/stress_test_$TIMESTAMP.log"
INTERVAL=1  # Collection interval in seconds
BLOCKCHAIN_PORTS=(9944 9945 9946)  # Default blockchain node ports

# Create metrics directory
mkdir -p "$METRICS_DIR"

# Function to log with timestamp
log_with_timestamp() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

# Function to get CPU usage
get_cpu_usage() {
    # Get CPU usage percentage (excluding idle)
    cpu_usage=$(top -l 1 -n 0 | grep "CPU usage" | awk '{print $3}' | sed 's/%//')
    echo "$cpu_usage"
}

# Function to get memory usage
get_memory_usage() {
    # Get memory info for macOS
    memory_info=$(vm_stat | head -5)
    total_pages=$(echo "$memory_info" | grep "Pages free" | awk '{print $3}' | tr -d '.')
    free_pages=$(echo "$memory_info" | grep "Pages free" | awk '{print $3}' | tr -d '.')
    inactive_pages=$(echo "$memory_info" | grep "Pages inactive" | awk '{print $3}' | tr -d '.')
    
    # Calculate memory usage percentage (simplified)
    memory_pressure=$(memory_pressure 2>/dev/null | grep "System-wide memory free percentage" | awk '{print $5}' | tr -d '%' || echo "N/A")
    echo "$memory_pressure"
}

# Function to get disk I/O
get_disk_io() {
    # Get disk I/O statistics
    iostat -d 1 1 | tail -n +3 | awk 'NR==1 {print "Read:", $3, "Write:", $4}'
}

# Function to get network I/O
get_network_io() {
    # Get network statistics
    netstat -ib | grep -E "en0|en1" | head -1 | awk '{print "Bytes In:", $7, "Bytes Out:", $10}'
}

# Function to check blockchain node processes
check_blockchain_processes() {
    local count=0
    for port in "${BLOCKCHAIN_PORTS[@]}"; do
        if lsof -i :$port >/dev/null 2>&1; then
            ((count++))
        fi
    done
    echo "$count"
}

# Function to get blockchain node resource usage
get_blockchain_resource_usage() {
    local total_cpu=0
    local total_memory=0
    local process_count=0
    
    for port in "${BLOCKCHAIN_PORTS[@]}"; do
        # Find process using the port
        pid=$(lsof -ti :$port 2>/dev/null)
        if [ -n "$pid" ]; then
            # Get CPU and memory usage for the process
            if ps -p $pid > /dev/null 2>&1; then
                cpu_mem=$(ps -p $pid -o %cpu,%mem --no-headers 2>/dev/null)
                if [ -n "$cpu_mem" ]; then
                    cpu=$(echo $cpu_mem | awk '{print $1}')
                    mem=$(echo $cpu_mem | awk '{print $2}')
                    total_cpu=$(echo "$total_cpu + $cpu" | bc 2>/dev/null || echo "$total_cpu")
                    total_memory=$(echo "$total_memory + $mem" | bc 2>/dev/null || echo "$total_memory")
                    ((process_count++))
                fi
            fi
        fi
    done
    
    echo "Processes: $process_count, Total CPU: ${total_cpu}%, Total Memory: ${total_memory}%"
}

# Function to test network connectivity to blockchain nodes
test_blockchain_connectivity() {
    local active_nodes=0
    for port in "${BLOCKCHAIN_PORTS[@]}"; do
        if nc -z localhost $port 2>/dev/null; then
            ((active_nodes++))
        fi
    done
    echo "$active_nodes"
}

# Function to get load average
get_load_average() {
    uptime | awk -F'load averages: ' '{print $2}'
}

# Function to get file descriptor usage
get_fd_usage() {
    lsof | wc -l
}

# Function to monitor blockchain-specific metrics
monitor_blockchain_metrics() {
    log_with_timestamp "=== BLOCKCHAIN METRICS ==="
    
    # Check active blockchain processes
    local active_processes=$(check_blockchain_processes)
    log_with_timestamp "Active blockchain processes: $active_processes"
    
    # Check connectivity
    local connected_nodes=$(test_blockchain_connectivity)
    log_with_timestamp "Connected nodes: $connected_nodes"
    
    # Get blockchain process resource usage
    local blockchain_resources=$(get_blockchain_resource_usage)
    log_with_timestamp "Blockchain resource usage: $blockchain_resources"
}

# Function to monitor system metrics
monitor_system_metrics() {
    log_with_timestamp "=== SYSTEM METRICS ==="
    
    # CPU Usage
    local cpu_usage=$(get_cpu_usage)
    log_with_timestamp "CPU Usage: ${cpu_usage}%"
    
    # Memory Usage
    local memory_usage=$(get_memory_usage)
    log_with_timestamp "Memory Free: ${memory_usage}%"
    
    # Load Average
    local load_avg=$(get_load_average)
    log_with_timestamp "Load Average: $load_avg"
    
    # Disk I/O
    local disk_io=$(get_disk_io)
    log_with_timestamp "Disk I/O: $disk_io"
    
    # Network I/O
    local network_io=$(get_network_io)
    log_with_timestamp "Network I/O: $network_io"
    
    # File Descriptors
    local fd_count=$(get_fd_usage)
    log_with_timestamp "Open File Descriptors: $fd_count"
}

# Function to generate summary report
generate_summary() {
    local test_duration=$1
    log_with_timestamp "=== STRESS TEST SUMMARY ==="
    log_with_timestamp "Test Duration: ${test_duration} seconds"
    log_with_timestamp "Metrics collected every ${INTERVAL} seconds"
    log_with_timestamp "Log file: $LOG_FILE"
    
    # Calculate averages (simplified)
    log_with_timestamp "Summary statistics available in: $LOG_FILE"
}

# Function to start monitoring
start_monitoring() {
    local duration=${1:-60}  # Default 60 seconds
    
    log_with_timestamp "Starting stress test metrics collection"
    log_with_timestamp "Duration: ${duration} seconds"
    log_with_timestamp "Interval: ${INTERVAL} seconds"
    log_with_timestamp "Monitoring blockchain ports: ${BLOCKCHAIN_PORTS[*]}"
    
    local start_time=$(date +%s)
    local end_time=$((start_time + duration))
    local iteration=0
    
    while [ $(date +%s) -lt $end_time ]; do
        ((iteration++))
        log_with_timestamp "--- Iteration $iteration ---"
        
        monitor_system_metrics
        monitor_blockchain_metrics
        
        log_with_timestamp ""
        sleep $INTERVAL
    done
    
    generate_summary $duration
}

# Function to run with blockchain test
run_with_test() {
    local test_command="$1"
    local monitor_duration=${2:-60}
    
    log_with_timestamp "Starting blockchain test with monitoring"
    log_with_timestamp "Test command: $test_command"
    
    # Start monitoring in background
    start_monitoring $monitor_duration &
    local monitor_pid=$!
    
    # Run the blockchain test
    eval "$test_command"
    local test_exit_code=$?
    
    # Wait for monitoring to complete
    wait $monitor_pid
    
    log_with_timestamp "Test completed with exit code: $test_exit_code"
    return $test_exit_code
}

# Function to analyze metrics from log file
analyze_metrics() {
    local log_file="$1"
    
    if [ ! -f "$log_file" ]; then
        echo "Log file not found: $log_file"
        return 1
    fi
    
    echo "=== METRICS ANALYSIS ==="
    echo "Log file: $log_file"
    
    # CPU usage analysis
    echo "CPU Usage Statistics:"
    grep "CPU Usage:" "$log_file" | awk -F': ' '{print $2}' | sed 's/%//' | awk '
    {
        sum += $1; count++; 
        if(NR==1 || $1 > max) max = $1; 
        if(NR==1 || $1 < min) min = $1
    } 
    END {
        if(count > 0) printf "  Average: %.2f%%, Min: %.2f%%, Max: %.2f%%\n", sum/count, min, max
    }'
    
    # Memory analysis
    echo "Memory Statistics:"
    grep "Memory Free:" "$log_file" | awk -F': ' '{print $2}' | sed 's/%//' | awk '
    {
        sum += $1; count++; 
        if(NR==1 || $1 > max) max = $1; 
        if(NR==1 || $1 < min) min = $1
    } 
    END {
        if(count > 0) printf "  Average Free: %.2f%%, Min Free: %.2f%%, Max Free: %.2f%%\n", sum/count, min, max
    }'
    
    # Blockchain process analysis
    echo "Blockchain Process Statistics:"
    grep "Active blockchain processes:" "$log_file" | awk -F': ' '{print $2}' | sort | uniq -c | awk '{print "  " $2 " processes active: " $1 " times"}'
    
    # Network connectivity analysis
    echo "Network Connectivity:"
    grep "Connected nodes:" "$log_file" | awk -F': ' '{print $2}' | sort | uniq -c | awk '{print "  " $2 " nodes connected: " $1 " times"}'
}

# Main script logic
case "${1:-help}" in
    "start")
        duration=${2:-60}
        start_monitoring $duration
        ;;
    "test")
        test_cmd="$2"
        duration=${3:-60}
        if [ -z "$test_cmd" ]; then
            echo "Usage: $0 test \"<command>\" [duration]"
            exit 1
        fi
        run_with_test "$test_cmd" $duration
        ;;
    "analyze")
        log_file="$2"
        if [ -z "$log_file" ]; then
            echo "Usage: $0 analyze <log_file>"
            exit 1
        fi
        analyze_metrics "$log_file"
        ;;
    "help"|*)
        cat << EOF
Blockchain Stress Test Metrics Collection Script

Usage:
    $0 start [duration]              - Start monitoring for specified duration (default: 60s)
    $0 test "command" [duration]     - Run command while monitoring (default: 60s)
    $0 analyze <log_file>           - Analyze metrics from log file
    $0 help                         - Show this help

Examples:
    $0 start 120                                    # Monitor for 2 minutes
    $0 test "node system_remark.js stress -d 30000" 45   # Run stress test with monitoring
    $0 analyze ./stress_metrics/stress_test_20231201_143022.log   # Analyze results

Output:
    Metrics are saved to: $METRICS_DIR/stress_test_TIMESTAMP.log
    
Monitored Metrics:
    - CPU Usage
    - Memory Usage  
    - Disk I/O
    - Network I/O
    - Load Average
    - File Descriptors
    - Blockchain Process Status
    - Node Connectivity
EOF
        ;;
esac