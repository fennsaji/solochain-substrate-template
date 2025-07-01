# Manual Testing Procedure for Off-chain Worker Auto-Rebase

This document provides a comprehensive step-by-step procedure to manually test the off-chain worker and automatic rebase functionality in the Substrate solochain template.

## Overview

The off-chain worker monitors blockchain growth and automatically triggers rebase operations at configured intervals. In the development environment, it's configured to trigger every 1,000 blocks (~8 minutes at 500ms block time).

## Prerequisites

### 1. Build the Node

```bash
cargo build --release
```

### 2. Verify Configuration

The development environment is configured for:
- ✅ Auto-rebase **enabled** 
- ⏰ Rebase interval: **1,000 blocks** (~8 minutes at 500ms blocks)
- 🔄 Off-chain worker runs on every block

## Testing Steps

### Step 1: Start the Node with Development Chain

```bash
# Start with clean state (recommended for testing)
./target/release/solochain-template-node purge-chain --dev

# Start the development node with detailed logging
RUST_LOG=runtime::offchain_simple=debug,sc_offchain=debug,substrate=info \
./target/release/solochain-template-node --dev
```

**Expected Output:**
- Node starts successfully
- You should see off-chain worker logs (if any appear in std builds)
- Block production begins at ~500ms intervals

### Step 2: Monitor Current Block Progress

Open a second terminal and use the RPC to check current block number:

```bash
# Check current block number
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"chain_getHeader"
}' http://localhost:9944/ | jq '.result.number'
```

**Expected Output:**
```json
"0x64"  // Example: block 100 in hex
```

**Convert hex to decimal:**
```bash
# Convert hex block number to decimal
echo $((0x64))  # Returns: 100
```

### Step 3: Check Rebase Configuration via RPC

Test the rebase API endpoints:

```bash
# Check if auto-rebase is enabled
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_isAutoRebaseEnabled"
}' http://localhost:9944/

# Check rebase interval
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_getRebaseInterval"  
}' http://localhost:9944/

# Check next rebase block
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_getNextRebaseBlock"
}' http://localhost:9944/
```

**Expected Output:**
```json
true        // Auto-rebase enabled
1000        // Rebase interval: 1000 blocks  
"0x3e8"     // Next rebase at block 1000 (hex)
```

### Step 4: Monitor Off-chain Worker Activity

Since we're in a no_std environment, direct logging is limited. Monitor via:

#### 4.1 Block Production Rate
```bash
# Watch block numbers increase every 2 seconds
watch -n 2 'curl -s -H "Content-Type: application/json" -d "{\"id\":1, \"jsonrpc\":\"2.0\", \"method\":\"chain_getHeader\"}" http://localhost:9944/ | jq -r ".result.number" | xargs printf "%d\n"'
```

#### 4.2 Create Monitoring Script
```bash
# Create a monitoring script
cat > monitor_blocks.sh << 'EOF'
#!/bin/bash
while true; do
  CURRENT=$(curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"chain_getHeader"}' http://localhost:9944/ | jq -r '.result.number' | xargs printf "%d\n" 2>/dev/null)
  if [ ! -z "$CURRENT" ]; then
    REMAINING=$((1000 - CURRENT))
    echo "$(date '+%H:%M:%S') - Block: $CURRENT, Remaining until rebase: $REMAINING"
    if [ $CURRENT -ge 1000 ]; then
      echo "🎯 REBASE INTERVAL REACHED!"
      break
    fi
  fi
  sleep 5
done
EOF

chmod +x monitor_blocks.sh
./monitor_blocks.sh
```

### Step 5: Test Fast Block Progression (Optional)

To speed up testing, you can modify the block time temporarily:

#### 5.1 Stop Current Node
```bash
# In the node terminal, press Ctrl+C
```

#### 5.2 Edit Block Time
```bash
# Edit runtime/src/lib.rs
# Change MILLI_SECS_PER_BLOCK from 500 to 100
sed -i.bak 's/MILLI_SECS_PER_BLOCK: u64 = 500/MILLI_SECS_PER_BLOCK: u64 = 100/' runtime/src/lib.rs
```

#### 5.3 Rebuild and Restart
```bash
cargo build --release
./target/release/solochain-template-node purge-chain --dev
./target/release/solochain-template-node --dev
```

#### 5.4 Restore Original Block Time (After Testing)
```bash
# Restore original block time
mv runtime/src/lib.rs.bak runtime/src/lib.rs
cargo build --release
```

### Step 6: Wait for Rebase Trigger Point

Monitor until you reach close to block 1000:

```bash
# Enhanced monitoring script with countdown
cat > countdown_monitor.sh << 'EOF'
#!/bin/bash
echo "🔍 Monitoring for rebase trigger at block 1000..."
while true; do
  CURRENT=$(curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"chain_getHeader"}' http://localhost:9944/ | jq -r '.result.number' | xargs printf "%d\n" 2>/dev/null)
  
  if [ ! -z "$CURRENT" ] && [ "$CURRENT" -gt 0 ]; then
    REMAINING=$((1000 - CURRENT))
    PERCENTAGE=$(( (CURRENT * 100) / 1000 ))
    
    printf "\r$(date '+%H:%M:%S') - Block: %4d/1000 [%3d%%] - Remaining: %4d" $CURRENT $PERCENTAGE $REMAINING
    
    if [ $CURRENT -ge 995 ]; then
      echo ""
      echo "⚠️  APPROACHING REBASE TRIGGER! ($REMAINING blocks remaining)"
    fi
    
    if [ $CURRENT -ge 1000 ]; then
      echo ""
      echo "🎯 REBASE INTERVAL REACHED AT BLOCK $CURRENT!"
      echo "📊 Testing rebase functionality..."
      break
    fi
  fi
  sleep 2
done
EOF

chmod +x countdown_monitor.sh
./countdown_monitor.sh
```

### Step 7: Verify Rebase Trigger (At Block 1000+)

When you reach block 1000, the off-chain worker should trigger a rebase. Check:

#### 7.1 Test State Export Functionality
```bash
echo "🧪 Testing state export..."
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_exportState"
}' http://localhost:9944/ | jq '.'
```

#### 7.2 Check Rebase Metadata
```bash
echo "📋 Checking rebase metadata..."
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_getRebaseMetadata"
}' http://localhost:9944/ | jq '.'
```

**Expected Behavior:**
- ✅ State export should return blockchain state data
- ✅ Metadata should show updated rebase count and next scheduled rebase at block 2000

### Step 8: Test Complete Rebase Functionality

#### 8.1 Export Current State
```bash
echo "💾 Exporting current blockchain state..."
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_exportState"
}' http://localhost:9944/ > exported_state.json

# Check if export was successful
if [ -s exported_state.json ]; then
  echo "✅ State exported successfully ($(wc -c < exported_state.json) bytes)"
  echo "📄 Preview:"
  head -c 200 exported_state.json && echo "..."
else
  echo "❌ State export failed"
fi
```

#### 8.2 Test State Root Validation
```bash
echo "🔐 Testing state root validation..."
# Note: You'll need to extract the actual state root from the export
# For demonstration, we'll test with a placeholder
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_validateStateRoot",
  "params":["0x0000000000000000000000000000000000000000000000000000000000000000"]
}' http://localhost:9944/ | jq '.'
```

#### 8.3 Check Next Rebase Schedule
```bash
echo "📅 Checking next rebase schedule..."
NEXT_REBASE=$(curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"rebase_getNextRebaseBlock"}' http://localhost:9944/ | jq -r '.result')
echo "Next rebase scheduled for block: $NEXT_REBASE"

# Convert to decimal
NEXT_DECIMAL=$(echo $((NEXT_REBASE)))
echo "Next rebase at block: $NEXT_DECIMAL (should be ~2000)"
```

### Step 9: Monitor Node Logs

In the node terminal, watch for:

```bash
# Look for these log patterns:
# - Block production: "💫 Imported"
# - Off-chain worker activity (if visible)
# - Any runtime events
# - Storage operations
# - Consensus messages

# You can also check specific log levels:
RUST_LOG=debug ./target/release/solochain-template-node --dev
```

### Step 10: Test Manual Override (Optional)

Test manual rebase functionality if needed:

```bash
echo "🔧 Testing manual rebase trigger..."
curl -H "Content-Type: application/json" -d '{
  "id":1, 
  "jsonrpc":"2.0", 
  "method":"rebase_forceManualRebase"
}' http://localhost:9944/ | jq '.'
```

## Verification Checklist

Use this checklist to verify all functionality:

- [ ] **Node Startup**: Node starts without errors
- [ ] **Block Production**: Blocks are produced at regular intervals  
- [ ] **RPC Endpoints**: All rebase RPC calls return expected values  
- [ ] **Auto-Rebase Config**: Shows enabled with 1000 block interval  
- [ ] **State Export**: Can export blockchain state successfully  
- [ ] **Rebase Trigger**: At block 1000, rebase logic activates  
- [ ] **Metadata Updates**: Rebase metadata reflects new state  
- [ ] **Next Schedule**: Next rebase scheduled for block 2000  
- [ ] **Performance**: No significant performance degradation
- [ ] **Error Handling**: Graceful handling of edge cases

## Expected Timeline

| Block Range | Expected Behavior |
|-------------|-------------------|
| 0-999 | Normal operation, off-chain worker monitors progress |
| 1000+ | **Rebase trigger activates**, metadata updates |
| 1000-1999 | Normal operation with updated schedule |
| 2000+ | Next rebase trigger (if node still running) |

## Troubleshooting

### RPC Endpoints Fail
```bash
# Check that runtime compiled successfully
cargo check -p solochain-template-runtime

# Verify RPC registration in node/src/rpc.rs
grep -n "rebase" node/src/rpc.rs

# Test basic RPC connectivity
curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"system_chain"}' http://localhost:9944/
```

### Off-chain Worker Doesn't Trigger
```bash
# Verify auto-rebase is enabled
curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"rebase_isAutoRebaseEnabled"}' http://localhost:9944/ | jq '.result'

# Check runtime includes off-chain worker
grep -n "RebaseWorker" runtime/src/lib.rs

# Verify environment configuration
grep -A5 -B5 "REBASE_AUTO_ENABLED" runtime/src/configs/environments.rs
```

### State Export Fails
```bash
# Test with basic state query first
curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"state_getRuntimeVersion"}' http://localhost:9944/

# Check runtime API implementation
grep -n "export_state" runtime/src/apis.rs

# Verify storage access
curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"state_getStorageSize"}' http://localhost:9944/
```

### Performance Issues
```bash
# Monitor system resources
htop

# Check block production rate
curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"chain_getHeader"}' http://localhost:9944/ | jq '.result.number' && sleep 10 && curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method":"chain_getHeader"}' http://localhost:9944/ | jq '.result.number'

# Monitor off-chain worker impact
RUST_LOG=sc_offchain=debug ./target/release/solochain-template-node --dev
```

## Environment Variations

### Development (Default)
- Auto-rebase: **Enabled**
- Interval: **1,000 blocks** (~8 minutes)
- Logging: Full debug available

### Staging
- Auto-rebase: **Enabled** 
- Interval: **50,000 blocks** (~7 hours)
- Use: `cargo build --release --features staging`

### Production
- Auto-rebase: **Disabled** (governance only)
- Interval: **100,000 blocks** (~14 hours if enabled)
- Use: `cargo build --release --features production`

### Local Testnet
- Auto-rebase: **Disabled**
- Interval: **5,000 blocks**
- Use: `cargo build --release --features local-testnet`

## Success Metrics

A successful test should demonstrate:

1. ✅ **Automatic Monitoring**: Off-chain worker runs on each block
2. ✅ **Interval Tracking**: Correctly counts blocks since last rebase  
3. ✅ **Trigger Activation**: Rebase activates at configured interval
4. ✅ **State Export**: Successfully exports blockchain state
5. ✅ **Metadata Updates**: Tracks rebase history and schedules
6. ✅ **Performance**: No blocking of main runtime operations
7. ✅ **Environment Compliance**: Respects configuration settings

## Next Steps

After successful testing:

1. **Integration Testing**: Test with state archival and genesis creation
2. **Network Testing**: Test with multiple nodes
3. **Governance Integration**: Connect with governance-driven rebases
4. **Archive Storage**: Integrate with IPFS or cloud storage
5. **Production Deployment**: Configure for live network use

---

**Note**: This testing procedure verifies the foundation of the Genesis Rebase Strategy. The off-chain worker provides automatic monitoring and triggering, ready for integration with complete rebase workflows including state archival, genesis creation, and network coordination.