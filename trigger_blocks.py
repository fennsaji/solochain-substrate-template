#!/usr/bin/env python3
"""
Script to trigger block production for testing off-chain workers
Sends periodic transactions to trigger MICC event-driven consensus
"""

import requests
import json
import time
import sys

def get_current_block():
    """Get current block number"""
    payload = {
        "jsonrpc": "2.0",
        "method": "chain_getHeader",
        "params": [],
        "id": 1
    }
    try:
        response = requests.post("http://localhost:9944/", json=payload, timeout=5)
        if response.status_code == 200:
            result = response.json()
            if "result" in result and result["result"]:
                block_hex = result["result"]["number"]
                return int(block_hex, 16)
    except Exception as e:
        print(f"Error getting block number: {e}")
    return None

def send_system_remark(message):
    """Send a system remark transaction to trigger block production"""
    # Create a simple system.remark call
    # This is a no-op transaction that doesn't change state but triggers block production
    remark_call = f"0x000008{len(message.encode().hex())//2:02x}{message.encode().hex()}"
    
    payload = {
        "jsonrpc": "2.0", 
        "method": "author_submitExtrinsic",
        "params": [remark_call],
        "id": 1
    }
    
    try:
        response = requests.post("http://localhost:9944/", json=payload, timeout=5)
        if response.status_code == 200:
            result = response.json()
            if "result" in result:
                return result["result"]
            elif "error" in result:
                print(f"Transaction error: {result['error']}")
        return None
    except Exception as e:
        print(f"Error sending transaction: {e}")
        return None

def main():
    print("🚀 Starting block production trigger for off-chain worker testing...")
    print("📊 This will send periodic transactions to trigger MICC block production")
    print("⏱️  Sending a transaction every 10 seconds")
    print("🛑 Press Ctrl+C to stop\n")
    
    counter = 0
    last_block = get_current_block()
    
    if last_block is None:
        print("❌ Could not connect to node. Make sure it's running on localhost:9944")
        sys.exit(1)
        
    print(f"📦 Starting at block #{last_block}")
    
    try:
        while True:
            counter += 1
            message = f"trigger-{counter}-{int(time.time())}"
            
            print(f"📤 Sending transaction #{counter}: system.remark('{message}')")
            tx_hash = send_system_remark(message)
            
            if tx_hash:
                print(f"✅ Transaction submitted: {tx_hash}")
            else:
                print("❌ Transaction failed")
            
            # Wait a moment then check block number
            time.sleep(2)
            current_block = get_current_block()
            
            if current_block and current_block > last_block:
                blocks_produced = current_block - last_block
                print(f"🎉 New block(s) produced! #{last_block} -> #{current_block} (+{blocks_produced})")
                
                # Check if we're at a rebase interval (every 100 blocks)
                if current_block % 100 == 0:
                    print(f"🎯 REBASE INTERVAL REACHED at block #{current_block}!")
                    print("   Off-chain worker should trigger automatic rebase now...")
                
                last_block = current_block
            else:
                print("⏳ Waiting for block production...")
            
            print(f"💤 Waiting 10 seconds before next transaction...\n")
            time.sleep(8)  # Total 10 seconds with the 2 second wait above
            
    except KeyboardInterrupt:
        final_block = get_current_block()
        if final_block:
            total_blocks = final_block - (last_block if counter == 1 else last_block)
            print(f"\n🛑 Stopped. Final block: #{final_block}")
            print(f"📊 Produced {total_blocks} blocks during testing")
        print("👋 Goodbye!")

if __name__ == "__main__":
    main()