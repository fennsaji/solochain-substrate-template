# Blockchain Scripts

Production-ready scripts for running your MICC consensus blockchain.

## 🚀 Quick Start

```bash
# 1. Build the blockchain
cargo build --release

# 2. Start the blockchain
./scripts/start-blockchain.sh

# 3. Connect via browser
# https://polkadot.js.org/apps/?rpc=ws://localhost:9944

# 4. Test the network (optional)
./scripts/test-network.sh

# 5. Stop the blockchain
./scripts/stop-nodes.sh
```

## 📁 Scripts

### `start-blockchain.sh` ⭐ **RECOMMENDED**
**Single node blockchain (most stable)**
- Single Alice node in development mode
- Fee-free transactions enabled
- RPC API on http://localhost:9944
- WebSocket on ws://localhost:9944
- Event-driven MICC consensus
- Automatic block production

### `start-3-nodes-simple.sh` 🔗 **MULTI-NODE**
**3 independent nodes for testing**
- Three separate development nodes (Alice, Bob, Charlie)
- Each node runs independently on ports 9944, 9945, 9946
- Fee-free transactions on all nodes
- Perfect for testing multi-node scenarios
- No complex consensus synchronization

### `test-network.sh` 🧪
**Comprehensive testing suite**
- Node connectivity tests
- Block production verification  
- RPC API functionality tests
- Balance and metadata queries
- Health monitoring

### `stop-nodes.sh` 🛑
**Clean shutdown utility**
```bash
./scripts/stop-nodes.sh        # Stop blockchain, keep data
./scripts/stop-nodes.sh --clean # Stop blockchain, remove all data
```

## ✅ Verified Working Features

- ✅ **Fee-free transactions** - No transaction fees required
- ✅ **MICC consensus** - Custom event-driven consensus working
- ✅ **Block production** - Automatic block creation on transactions
- ✅ **JSON-RPC API** - Full Substrate RPC interface
- ✅ **WebSocket** - Real-time connections
- ✅ **Polkadot-JS Apps** - Browser wallet integration
- ✅ **Account management** - Alice, Bob, Charlie accounts
- ✅ **Balance transfers** - Send tokens between accounts

## 🔗 Network Information

| Component | Value |
|-----------|-------|
| **RPC HTTP** | http://localhost:9944 |
| **WebSocket** | ws://localhost:9944 |
| **Polkadot-JS** | https://polkadot.js.org/apps/?rpc=ws://localhost:9944 |
| **Chain** | Development |
| **Consensus** | MICC (Event-driven) |
| **Finality** | GRANDPA |
| **Data Dir** | `../blockchain-data/` |
| **Logs** | `../blockchain-data/alice.log` |

## 🧪 Testing Examples

### Check Node Health
```bash
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' \
  http://localhost:9944
```

### Get Latest Block
```bash
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getBlock","params":[],"id":1}' \
  http://localhost:9944
```

### Send Fee-free Transaction
1. Open https://polkadot.js.org/apps/?rpc=ws://localhost:9944
2. Go to Accounts → Transfer
3. Send tokens from Alice to Bob (no fees required!)

## 🔧 Troubleshooting

### Port Already in Use
```bash
# Kill processes using port 9944
lsof -ti:9944 | xargs kill -9
```

### Node Won't Start
```bash
# Clean all data and restart
./scripts/stop-nodes.sh --clean
./scripts/start-blockchain.sh
```

### Check Logs
```bash
tail -f ../blockchain-data/alice.log
```

## 🎯 Production Notes

⚠️ **Development Only**: These scripts use development keys and are not suitable for production.

For production deployment:
- Generate secure validator keys  
- Implement proper network security
- Configure monitoring and alerting
- Use hardware security modules (HSM)
- Set up proper firewall rules