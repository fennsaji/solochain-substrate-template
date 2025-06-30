# Blockchain Testing Tool

A comprehensive testing suite for MetaMUI blockchain to validate event-driven block production, transaction priority handling, and overall system performance.

## Features

- **Multiple Test Scenarios**: Single, serial, parallel, priority, stress, and burst testing
- **Priority Testing**: Test high-priority transactions with immediate block production
- **Multi-Account Support**: Test with Alice, Bob, Charlie, Dave, Eve, and Ferdie
- **Detailed Statistics**: Transaction success rates, timing, and performance metrics
- **CLI Interface**: Easy-to-use command-line interface with flexible options

## Installation

```bash
cd test_blockchain
npm install
```

## Usage

### Basic Commands

```bash
# Show help
npm run help

# Basic single transaction test
npm run test:single

# Serial transactions test
npm run test:serial

# Parallel transactions test
npm run test:parallel

# Priority testing
npm run test:priority

# Stress test
npm run test:stress

# Burst test
npm run test:burst
```

### Advanced Usage

#### Single Transaction
```bash
# Send single transaction with specific options
node system_remark.js single \
  --account alice \
  --remark "My test transaction" \
  --priority high

# Available accounts: alice, bob, charlie, dave, eve, ferdie
# Available priorities: low, normal, high, critical, max
```

#### Serial Transactions
```bash
# Send 10 transactions in sequence with 2-second delays
node system_remark.js serial \
  --count 10 \
  --account bob \
  --delay 2000 \
  --priority normal
```

#### Parallel Transactions
```bash
# Send 5 transactions from each of 4 accounts simultaneously
node system_remark.js parallel \
  --count 5 \
  --accounts alice,bob,charlie,dave \
  --priority high
```

#### Priority Testing
```bash
# Test mixed priority transactions (tests event-driven priority handling)
node system_remark.js priority --count 20
```

#### Stress Testing
```bash
# Run stress test for 2 minutes with 50ms intervals
node system_remark.js stress \
  --duration 120000 \
  --interval 50
```

#### Burst Testing
```bash
# Send 30 transactions in bursts, 5 bursts total, 10 seconds apart
node system_remark.js burst \
  --size 30 \
  --count 5 \
  --interval 10000
```

### Custom Endpoint
```bash
# Connect to different blockchain endpoint
node system_remark.js single \
  --endpoint ws://192.168.1.100:9944 \
  --account alice
```

## Test Scenarios

### 1. Event-Driven Validation Tests

#### Test Immediate Block Production
```bash
# Test that transactions trigger immediate block production
npm run test:single
npm run test:serial -- --count 5 --delay 3000
```

#### Test Priority Handling
```bash
# Test high-priority transactions get immediate processing
node system_remark.js single --priority critical
node system_remark.js priority --count 15
```

#### Test Collection Windows
```bash
# Test transaction batching in collection windows
node system_remark.js parallel --count 3 --accounts alice,bob
```

### 2. Performance Tests

#### Network Load Testing
```bash
# Light load
node system_remark.js stress --duration 30000 --interval 1000

# Medium load
node system_remark.js stress --duration 60000 --interval 500

# Heavy load
node system_remark.js stress --duration 30000 --interval 100
```

#### Burst Capacity Testing
```bash
# Small bursts
node system_remark.js burst --size 10 --count 3

# Large bursts
node system_remark.js burst --size 50 --count 3 --interval 5000
```

### 3. Multi-Account Testing

#### Concurrent Users Simulation
```bash
# Simulate 6 users sending transactions simultaneously
node system_remark.js parallel \
  --count 10 \
  --accounts alice,bob,charlie,dave,eve,ferdie \
  --priority normal
```

#### Mixed Priority Multi-User
```bash
# Test priority handling with multiple users
node system_remark.js priority --count 30
```

## Validating Event-Driven Features

### 1. Zero-Delay Response Testing
Run single transactions and verify blocks are produced immediately:
```bash
npm run test:single
```
**Expected**: Block production within 1-2 seconds of transaction submission.

### 2. Priority Fast-Track Testing
Test high-priority transactions bypass collection windows:
```bash
node system_remark.js single --priority critical
```
**Expected**: Immediate block production for high-priority transactions.

### 3. Collection Window Testing
Test transaction batching behavior:
```bash
node system_remark.js parallel --count 5 --accounts alice,bob,charlie
```
**Expected**: Multiple transactions batched into single blocks.

### 4. Empty Block Prevention Testing
Run tests and verify no empty blocks are produced during active periods:
```bash
npm run test:stress -- --duration 60000 --interval 200
```
**Expected**: No empty blocks during continuous transaction flow.

## Monitoring

The tool provides detailed statistics including:
- Total transactions sent
- Success/failure counts
- Success rate percentage
- Test duration
- Transactions per second (TPS)

### Example Output
```
📊 Test Statistics:
   Total transactions: 50
   Successful: 49 ✅
   Failed: 1 ❌
   Success rate: 98.0%
   Duration: 12345ms
   TPS: 4.05 transactions/second
```

## Troubleshooting

### Connection Issues
```bash
# Check if blockchain is running
curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "system_health"}' http://localhost:9933
```

### Performance Issues
- Reduce transaction frequency with higher `--interval` values
- Use fewer parallel accounts
- Check system resources during stress tests

### Transaction Failures
- Check account balances (ensure sufficient funds for transaction fees)
- Verify blockchain is synced and accepting transactions
- Check transaction priority limits

## Integration with Development

### Recommended Testing Workflow

1. **Start Development Node**:
   ```bash
   ./target/debug/metamui-node --dev --tmp
   ```

2. **Basic Functionality Test**:
   ```bash
   npm run test:single
   ```

3. **Event-Driven Validation**:
   ```bash
   npm run test:priority
   ```

4. **Performance Baseline**:
   ```bash
   npm run test:stress -- --duration 30000
   ```

5. **Load Testing**:
   ```bash
   npm run test:burst
   ```

This testing tool is specifically designed to validate the event-driven block production features implemented in MetaMUI, including transaction priority handling, smart collection windows, and immediate response capabilities.