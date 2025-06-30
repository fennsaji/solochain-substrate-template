# TPS Testing and Throughput Analysis

This directory contains comprehensive tools for testing blockchain throughput, measuring transactions per second (TPS), and analyzing system performance during load testing.

## Scripts Overview

### 1. `throughput_analyzer.js`
Advanced TPS testing tool with multiple test modes:

**Features:**
- Baseline TPS testing with incremental load increases
- Concurrency testing across multiple accounts
- Maximum sustainable TPS discovery
- Load ramp testing (gradual increase)
- Priority impact analysis
- Comprehensive reporting with JSON and text outputs

**Usage:**
```bash
# Quick baseline test
node throughput_analyzer.js baseline --max-tps 30 --duration 20000

# Concurrency test with 6 accounts
node throughput_analyzer.js concurrency --accounts 6 --tps 10

# Find maximum sustainable TPS
node throughput_analyzer.js max-tps --initial 50

# Full comprehensive analysis
node throughput_analyzer.js full
```

### 2. `tps_monitor.sh`
Integrated TPS testing with system monitoring:

**Features:**
- Combines throughput testing with system performance monitoring
- Real-time CPU, memory, disk, and network monitoring
- Combined reports with both TPS and system metrics
- Multiple test profiles (quick, comprehensive, targeted)

**Usage:**
```bash
# Quick test with monitoring
./tps_monitor.sh quick

# Comprehensive analysis suite
./tps_monitor.sh comprehensive

# Monitor external test
./tps_monitor.sh monitor 60 my_test
```

### 3. `stress_metrics.sh`
System performance monitoring (integrated with TPS testing):

**Features:**
- CPU, memory, disk I/O monitoring
- Blockchain process tracking
- Network connectivity checks
- Metrics analysis and reporting

## Test Types Explained

### Baseline TPS Test
Tests single-account throughput by gradually increasing transaction rate.
- **Purpose:** Find maximum TPS for single account
- **Method:** Incremental TPS increases with success rate monitoring
- **Output:** TPS vs success rate curve

### Concurrency Test
Tests multi-account parallel transaction processing.
- **Purpose:** Measure scaling with multiple accounts
- **Method:** Multiple accounts sending transactions simultaneously
- **Output:** Total TPS and scaling efficiency

### Maximum TPS Test
Discovers the highest sustainable transaction rate.
- **Purpose:** Find absolute performance limits
- **Method:** Binary search approach with sustainability testing
- **Output:** Maximum sustainable TPS with specified success threshold

### Load Ramp Test
Gradually increases load to identify breaking points.
- **Purpose:** Understand performance degradation patterns
- **Method:** Stepped increase in TPS over time
- **Output:** Performance curve showing degradation points

### Priority Impact Test
Measures how transaction priorities affect throughput.
- **Purpose:** Understand priority mechanism impact
- **Method:** Same TPS with different priority levels
- **Output:** Priority vs performance comparison

## Quick Start

1. **Ensure blockchain node is running:**
   ```bash
   ./target/debug/metamui-node --dev --tmp
   ```

2. **Run quick TPS test:**
   ```bash
   ./tps_monitor.sh quick
   ```

3. **Run comprehensive analysis:**
   ```bash
   ./tps_monitor.sh comprehensive
   ```

## Interpreting Results

### Key Metrics

- **TPS (Transactions Per Second):** Actual achieved transaction rate
- **Success Rate:** Percentage of transactions that completed successfully
- **Transaction Time:** Average time from submission to block inclusion
- **Concurrency Scaling:** Efficiency of multi-account processing

### Performance Indicators

- **Good Performance:** Success rate > 95%, TPS close to target
- **Degraded Performance:** Success rate 80-95%, TPS below target
- **Overloaded:** Success rate < 80%, significant TPS reduction

### System Health Indicators

- **CPU Usage:** Should remain < 80% for sustainable operation
- **Memory Usage:** Monitor for memory leaks during extended tests
- **Network I/O:** High during transaction bursts, should stabilize
- **File Descriptors:** Monitor for resource leaks

## Output Files

Results are saved to configurable directories with timestamped files:

### Throughput Analysis Results
- `throughput_analysis_TIMESTAMP.json` - Complete test data
- `throughput_summary_TIMESTAMP.txt` - Human-readable summary

### System Monitoring Results
- `system_metrics_TIMESTAMP.log` - System performance logs
- `combined_report_TIMESTAMP.txt` - Integrated analysis report

### Report Structure
```
throughput_results/
├── throughput_analysis_20231201_143022.json
├── throughput_summary_20231201_143022.txt
├── system_metrics_baseline_20231201_143022.log
└── combined_report_baseline_20231201_143022.txt
```

## Example Test Scenarios

### Development Testing
```bash
# Quick validation
./tps_monitor.sh quick

# Single feature testing
node throughput_analyzer.js baseline --max-tps 20 --duration 15000
```

### Performance Benchmarking
```bash
# Full analysis
./tps_monitor.sh comprehensive

# Specific capacity testing
node throughput_analyzer.js max-tps --initial 50 --max-attempts 8
```

### Load Testing
```bash
# Sustained load test
node throughput_analyzer.js ramp --start-tps 10 --end-tps 100 --duration 120000

# Burst testing
node system_remark.js burst --size 50 --count 5
```

## Troubleshooting

### Common Issues

1. **Connection Errors:**
   - Ensure blockchain node is running
   - Check endpoint URL (default: ws://127.0.0.1:9944)
   - Verify WebSocket connectivity

2. **Low TPS Results:**
   - Check system resources (CPU, memory)
   - Verify node configuration
   - Monitor network latency

3. **High Failure Rates:**
   - Reduce target TPS
   - Check nonce management
   - Monitor node logs for errors

### Performance Optimization Tips

1. **For Higher TPS:**
   - Increase block time if possible
   - Optimize transaction pool settings
   - Use multiple accounts for parallel processing

2. **For Stability:**
   - Monitor system resources
   - Use conservative TPS targets
   - Implement proper error handling

3. **For Accuracy:**
   - Run longer test durations
   - Use multiple test iterations
   - Account for system warm-up time

## Best Practices

1. **Test Environment:**
   - Use dedicated test nodes
   - Ensure consistent system state
   - Monitor external load factors

2. **Test Design:**
   - Start with baseline tests
   - Gradually increase complexity
   - Document test parameters

3. **Result Analysis:**
   - Compare multiple test runs
   - Consider system variability
   - Focus on sustainable performance

## Integration with Existing Tools

The TPS testing tools integrate seamlessly with:
- `system_remark.js` - For additional test patterns
- `stress_metrics.sh` - For system monitoring
- Existing blockchain development workflow

For more detailed usage examples and advanced configurations, see the individual script help outputs:
```bash
node throughput_analyzer.js --help
./tps_monitor.sh --help
./stress_metrics.sh help
```